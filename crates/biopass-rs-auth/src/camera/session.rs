use self_cell::self_cell;
use std::time::{Duration, Instant};
use v4l::buffer::Type;
use v4l::io::mmap::Stream as MmapStream;
use v4l::prelude::Device;
use v4l::video::Capture;
use v4l::Format;

use super::controls::apply_camera_optimizations;
use super::decode::{decode_frame, unsupported_format_message};
use super::device::{open_device, select_format};
use super::ir::{capture_grey_ir_frame, GreyFrameLayout};
use super::stream::next_frame_before;
use super::{CameraRequest, FrameFormat, RgbFrame};

/// 实际格式 + 设备句柄的元数据，存为 owner 以便 dependent 可以借用。
struct SessionOwner {
    device: Device,
    format: FrameFormat,
    width: u32,
    height: u32,
    stride: u32,
    frame_read_timeout: Duration,
    max_dark_frames: u32,
    debug: bool,
}

impl std::fmt::Debug for SessionOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionOwner")
            .field("format", &self.format)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .field("frame_read_timeout", &self.frame_read_timeout)
            .field("max_dark_frames", &self.max_dark_frames)
            .field("debug", &self.debug)
            .finish()
    }
}

self_cell!(
    /// Long-lived camera capture session.
    ///
    /// `Device` (within `SessionOwner`) is opened once and kept open for the
    /// lifetime of the session. `MmapStream` borrows from `Device` via the
    /// `self_cell!` macro, which makes the self-referential layout sound
    /// without hand-rolled `unsafe`.
    pub struct CameraSession {
        owner: SessionOwner,
        #[covariant]
        dependent: MmapStream,
    }
);

const BUFFER_COUNT: u32 = 4;

impl CameraSession {
    /// Open the requested device, apply V4L2 format and (optionally) the
    /// auto-exposure optimizations, then start streaming.
    pub fn open(request: &CameraRequest) -> Result<Self, String> {
        let mut device = open_device(request)?;
        let preferred = select_format(&device, request)?;
        let actual = device
            .set_format(&Format::new(
                request.width,
                request.height,
                preferred.fourcc(),
            ))
            .map_err(|error| format!("Failed to set V4L2 format: {error}"))?;
        let actual_format = FrameFormat::from_fourcc(actual.fourcc)
            .ok_or_else(|| unsupported_format_message(actual.fourcc))?;

        if request.auto_optimize_camera {
            apply_camera_optimizations(&mut device, request.debug)?;
        }

        let owner = SessionOwner {
            device,
            format: actual_format,
            width: actual.width,
            height: actual.height,
            stride: actual.stride,
            frame_read_timeout: request.timeout,
            max_dark_frames: request.max_dark_frames,
            debug: request.debug,
        };

        CameraSession::try_new(owner, |owner| {
            MmapStream::with_buffers(&owner.device, Type::VideoCapture, BUFFER_COUNT)
                .map_err(|error| format!("Failed to create V4L2 mmap stream: {error}"))
        })
    }

    /// Discard the first `frames` frames so that auto-exposure / auto-white
    /// balance can converge before the caller starts using `next_frame`.
    pub fn warmup(&mut self, frames: u32) -> Result<(), String> {
        for _ in 0..frames {
            self.with_dependent_mut(|owner, stream| {
                next_frame_before(stream, owner.frame_read_timeout)
            })?;
        }
        Ok(())
    }

    /// Keep the stream active for `duration`, discarding frames while camera
    /// firmware converges exposure / gain.
    pub fn warmup_for(&mut self, duration: Duration) -> Result<(), String> {
        if duration.is_zero() {
            return Ok(());
        }

        let deadline = Instant::now() + duration;
        while Instant::now() < deadline {
            let timeout = deadline.saturating_duration_since(Instant::now());
            if timeout.is_zero() {
                break;
            }
            self.with_dependent_mut(|_owner, stream| next_frame_before(stream, timeout))?;
        }
        Ok(())
    }

    /// Read and decode the next frame from the open stream.
    pub fn next_frame(&mut self) -> Result<RgbFrame, String> {
        self.with_dependent_mut(|owner, stream| {
            let SessionOwner {
                format,
                width,
                height,
                stride,
                frame_read_timeout,
                max_dark_frames,
                debug,
                ..
            } = owner;
            let deadline = Instant::now() + *frame_read_timeout;
            let buffer = next_frame_before(stream, *frame_read_timeout)?;

            if *format == FrameFormat::Grey {
                return capture_grey_ir_frame(
                    stream,
                    &buffer,
                    GreyFrameLayout {
                        width: *width,
                        height: *height,
                        stride: *stride,
                    },
                    &deadline,
                    *max_dark_frames,
                    *debug,
                );
            }

            decode_frame(*format, *width, *height, *stride, &buffer)
        })
    }
}
