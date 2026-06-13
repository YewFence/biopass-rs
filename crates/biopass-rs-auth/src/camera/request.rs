use super::FrameFormat;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_WIDTH: u32 = 640;
const DEFAULT_HEIGHT: u32 = 480;
const DEFAULT_TIMEOUT_MS: u64 = 3000;
const DEFAULT_WARMUP_FRAMES: u32 = 5;
const DEFAULT_MAX_DARK_FRAMES: u32 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraRequest {
    pub device_path: Option<PathBuf>,
    pub width: u32,
    pub height: u32,
    pub preferred_formats: Vec<FrameFormat>,
    pub warmup_frames: u32,
    pub timeout: Duration,
    pub max_dark_frames: u32,
    pub auto_optimize_camera: bool,
    pub debug: bool,
}

impl Default for CameraRequest {
    fn default() -> Self {
        Self {
            device_path: None,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            // MJPEG first: it goes through the UVC controller's ISP (3DNR, AE,
            // AWB, sharpening) and looks dramatically cleaner than raw YUYV.
            // Falls back to raw formats only when the device cannot produce MJPEG.
            preferred_formats: vec![
                FrameFormat::Mjpeg,
                FrameFormat::Yuyv,
                FrameFormat::Nv12,
                FrameFormat::Grey,
            ],
            warmup_frames: DEFAULT_WARMUP_FRAMES,
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
            max_dark_frames: DEFAULT_MAX_DARK_FRAMES,
            auto_optimize_camera: true,
            debug: false,
        }
    }
}
