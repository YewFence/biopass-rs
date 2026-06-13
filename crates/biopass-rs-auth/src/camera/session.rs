use self_cell::self_cell;
use std::time::Duration;
use v4l::buffer::Type;
use v4l::io::mmap::Stream as MmapStream;
use v4l::prelude::Device;
use v4l::video::Capture;
use v4l::Format;

use super::controls::apply_camera_optimizations;
use super::decode::{decode_frame, unsupported_format_message};
use super::device::{open_device, select_format};
use super::stream::next_frame_before;
use super::{CameraRequest, FrameFormat, RgbFrame};

/// 实际格式 + 设备句柄的元数据，存为 owner 以便 dependent 可以借用。
struct SessionOwner {
    device: Device,
    format: FrameFormat,
    width: u32,
    height: u32,
    stride: u32,
}

impl std::fmt::Debug for SessionOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionOwner")
            .field("format", &self.format)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
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
const FRAME_READ_TIMEOUT: Duration = Duration::from_millis(1000);

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
            self.with_dependent_mut(|_owner, stream| {
                next_frame_before(stream, FRAME_READ_TIMEOUT)
            })?;
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
                ..
            } = owner;
            let buffer = next_frame_before(stream, FRAME_READ_TIMEOUT)?;
            decode_frame(*format, *width, *height, *stride, &buffer)
        })
    }
}
