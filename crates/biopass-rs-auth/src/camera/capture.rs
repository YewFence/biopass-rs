use super::controls::apply_camera_optimizations;
use super::decode::{decode_frame, unsupported_format_message};
use super::device::{open_device, select_format};
use super::ir::{capture_grey_ir_frame, GreyFrameLayout};
use super::stream::next_frame_before;
use super::{CameraRequest, FrameFormat, RgbFrame};
use std::time::Instant;
use v4l::buffer::Type;
use v4l::io::mmap::Stream as MmapStream;
use v4l::video::Capture;
use v4l::Format;

pub fn camera_available(request: &CameraRequest) -> bool {
    open_device(request)
        .and_then(|device| select_format(&device, request))
        .is_ok()
}

pub fn capture_rgb_frame(request: &CameraRequest) -> Result<RgbFrame, String> {
    let mut device = open_device(request)?;
    let format = select_format(&device, request)?;
    let actual = device
        .set_format(&Format::new(request.width, request.height, format.fourcc()))
        .map_err(|error| format!("Failed to set V4L2 format: {error}"))?;

    let actual_format = FrameFormat::from_fourcc(actual.fourcc)
        .ok_or_else(|| unsupported_format_message(actual.fourcc))?;

    // 在开始流式传输前启用所有自动优化
    if request.auto_optimize_camera {
        apply_camera_optimizations(&mut device, request.debug)?;
    }

    let mut stream = MmapStream::with_buffers(&device, Type::VideoCapture, 4)
        .map_err(|error| format!("Failed to create V4L2 mmap stream: {error}"))?;

    let deadline = Instant::now() + request.timeout;
    let mut latest = Vec::new();
    for index in 0..=request.warmup_frames {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| "Timed out waiting for V4L2 frame".to_string())?;
        let buffer = next_frame_before(&mut stream, remaining)?;
        if index == request.warmup_frames {
            latest = buffer;
        }
    }

    if actual_format == FrameFormat::Grey {
        return capture_grey_ir_frame(
            &mut stream,
            &latest,
            GreyFrameLayout {
                width: actual.width,
                height: actual.height,
                stride: actual.stride,
            },
            &deadline,
            request.max_dark_frames,
            request.debug,
        );
    }

    decode_frame(
        actual_format,
        actual.width,
        actual.height,
        actual.stride,
        &latest,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn missing_explicit_camera_is_unavailable() {
        let request = CameraRequest {
            device_path: Some(PathBuf::from("/dev/biopass-rs-missing-camera")),
            ..CameraRequest::default()
        };

        assert!(!camera_available(&request));
    }
}
