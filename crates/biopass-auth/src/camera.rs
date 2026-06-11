use jpeg_decoder::{Decoder, PixelFormat as JpegPixelFormat};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use v4l::buffer::Type;
use v4l::control::{Control, Value as ControlValue};
use v4l::io::mmap::Stream as MmapStream;
use v4l::io::traits::CaptureStream;
use v4l::prelude::*;
use v4l::video::Capture;
use v4l::{Format, FourCC};

const DEFAULT_WIDTH: u32 = 640;
const DEFAULT_HEIGHT: u32 = 480;
const DEFAULT_TIMEOUT_MS: u64 = 3000;
const DEFAULT_WARMUP_FRAMES: u32 = 5;
const DEFAULT_MAX_DARK_FRAMES: u32 = 5;
const DARK_IR_MEAN_THRESHOLD: f64 = 10.0;
const DARK_IR_MAX_THRESHOLD: u8 = 80;

#[derive(Debug, Clone, Copy, PartialEq)]
struct GreyFrameStats {
    mean: f64,
    min: u8,
    max: u8,
}

impl Default for GreyFrameStats {
    fn default() -> Self {
        Self {
            mean: 0.0,
            min: 0,
            max: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameFormat {
    Yuyv,
    Mjpeg,
    Nv12,
    Grey,
}

impl FrameFormat {
    pub fn fourcc(self) -> FourCC {
        match self {
            Self::Yuyv => FourCC::new(b"YUYV"),
            Self::Mjpeg => FourCC::new(b"MJPG"),
            Self::Nv12 => FourCC::new(b"NV12"),
            Self::Grey => FourCC::new(b"GREY"),
        }
    }

    pub fn from_fourcc(fourcc: FourCC) -> Option<Self> {
        match fourcc.repr {
            [b'Y', b'U', b'Y', b'V'] => Some(Self::Yuyv),
            [b'M', b'J', b'P', b'G'] => Some(Self::Mjpeg),
            [b'N', b'V', b'1', b'2'] => Some(Self::Nv12),
            [b'G', b'R', b'E', b'Y'] => Some(Self::Grey),
            _ => None,
        }
    }
}

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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoDevice {
    pub path: PathBuf,
    pub driver: String,
    pub card: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl RgbFrame {
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Result<Self, String> {
        let expected = width as usize * height as usize * 3;
        if data.len() != expected {
            return Err(format!(
                "RGB frame size mismatch: expected {expected} bytes, got {}",
                data.len()
            ));
        }

        Ok(Self {
            width,
            height,
            data,
        })
    }
}

pub fn list_video_devices() -> Vec<VideoDevice> {
    video_device_paths()
        .into_iter()
        .filter_map(|path| {
            let device = Device::with_path(&path).ok()?;
            let caps = device.query_caps().ok()?;
            Some(VideoDevice {
                path,
                driver: caps.driver,
                card: caps.card,
            })
        })
        .collect()
}

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
        apply_camera_optimizations(&mut device)?;
    }

    let mut stream = MmapStream::with_buffers(&mut device, Type::VideoCapture, 4)
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
            &actual,
            &deadline,
            request.max_dark_frames,
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

/// Captures an IR frame from a V4L2 GREY stream, skipping dark frames until
/// a sufficiently bright one is found. On timeout or max_dark_frames limit,
/// returns the last dark frame instead of failing completely.
fn capture_grey_ir_frame(
    stream: &mut MmapStream<'_>,
    warmup: &[u8],
    actual: &Format,
    deadline: &Instant,
    max_dark_frames: u32,
) -> Result<RgbFrame, String> {
    let width = actual.width;
    let height = actual.height;
    let stride = actual.stride;

    let mut skipped_dark_frames: u32 = 0;
    #[allow(unused_assignments)]
    let mut last_dark: Option<(GreyFrameStats, RgbFrame)> = None;

    let (stats, dark) = grey_frame_stats_and_dark(warmup, width, height, stride);
    if !dark {
        return decode_grey(width, height, stride, warmup);
    }
    skipped_dark_frames += 1;
    last_dark = Some((stats, decode_grey(width, height, stride, warmup)?));
    eprintln!(
        "FaceAuth: skipping dark IR frame from V4L2 GREY mean={:.2}, min={}, max={}, skipped={}",
        stats.mean, stats.min, stats.max, skipped_dark_frames
    );

    loop {
        if skipped_dark_frames >= max_dark_frames {
            if let Some((stats, frame)) = last_dark.take() {
                eprintln!(
                    "FaceAuth: Reached max dark frames limit ({}) for V4L2 GREY, \
                     returning last dark frame mean={:.2}, min={}, max={}",
                    max_dark_frames, stats.mean, stats.min, stats.max
                );
                return Ok(frame);
            }
        }

        if Instant::now() >= *deadline {
            if let Some((stats, frame)) = last_dark.take() {
                eprintln!(
                    "FaceAuth: Timed out waiting for non-dark V4L2 GREY frame after skipping \
                     {} dark frame(s), returning last dark frame mean={:.2}, min={}, max={}",
                    skipped_dark_frames, stats.mean, stats.min, stats.max
                );
                return Ok(frame);
            }
            return Err("Timed out waiting for V4L2 GREY frame".to_string());
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        let buffer = next_frame_before(stream, remaining)?;

        let (stats, dark) = grey_frame_stats_and_dark(&buffer, width, height, stride);
        if dark {
            skipped_dark_frames += 1;
            last_dark = Some((stats, decode_grey(width, height, stride, &buffer)?));
            eprintln!(
                "FaceAuth: skipping dark IR frame from V4L2 GREY mean={:.2}, min={}, max={}, skipped={}",
                stats.mean, stats.min, stats.max, skipped_dark_frames
            );
            continue;
        }

        eprintln!(
            "FaceAuth: returning V4L2 GREY IR frame mean={:.2}, min={}, max={}, skipped_dark={}",
            stats.mean, stats.min, stats.max, skipped_dark_frames
        );
        return decode_grey(width, height, stride, &buffer);
    }
}

fn grey_frame_stats_and_dark(
    data: &[u8],
    width: u32,
    height: u32,
    stride: u32,
) -> (GreyFrameStats, bool) {
    let stats = calculate_grey_frame_stats(data, width, height, stride);
    (stats, is_dark_ir_frame(stats))
}

fn calculate_grey_frame_stats(data: &[u8], width: u32, height: u32, stride: u32) -> GreyFrameStats {
    if width == 0 || height == 0 {
        return GreyFrameStats::default();
    }

    let width = width as usize;
    let height = height as usize;
    let stride = stride.max(width as u32) as usize;
    if data.len() < stride * height {
        return GreyFrameStats::default();
    }

    let mut min: u8 = 255;
    let mut max: u8 = 0;
    let mut sum: u64 = 0;
    for row in 0..height {
        let line = &data[row * stride..row * stride + width];
        for &value in line {
            sum += value as u64;
            if value < min {
                min = value;
            }
            if value > max {
                max = value;
            }
        }
    }

    let pixel_count = (width * height) as f64;
    GreyFrameStats {
        mean: (sum as f64) / pixel_count,
        min,
        max,
    }
}

fn is_dark_ir_frame(stats: GreyFrameStats) -> bool {
    stats.mean < DARK_IR_MEAN_THRESHOLD && stats.max < DARK_IR_MAX_THRESHOLD
}

fn open_device(request: &CameraRequest) -> Result<Device, String> {
    if let Some(path) = &request.device_path {
        return Device::with_path(path)
            .map_err(|error| format!("Failed to open V4L2 device {}: {error}", path.display()));
    }

    for path in video_device_paths() {
        if let Ok(device) = Device::with_path(&path) {
            return Ok(device);
        }
    }

    Device::new(0).map_err(|error| format!("Failed to open default V4L2 device: {error}"))
}

/// 应用相机控制参数以优化图像质量
///
/// 此函数会尽力启用以下功能:
/// - 自动白平衡 (AWB)
/// - 自动曝光 (光圈优先)
/// - 防闪烁 (50Hz)
/// - 宽动态范围 (背光补偿)
/// - 曝光优先 (动态帧率)
fn apply_camera_optimizations(device: &mut Device) -> Result<(), String> {
    // V4L2 控制常量
    const WHITE_BALANCE_AUTOMATIC: u32 = 0x0098_090c;
    const POWER_LINE_FREQUENCY: u32 = 0x0098_0918;
    const BACKLIGHT_COMPENSATION: u32 = 0x0098_091c;
    const AUTO_EXPOSURE: u32 = 0x009a_0901;
    const EXPOSURE_DYNAMIC_FRAMERATE: u32 = 0x009a_0903;

    // 自动白平衡
    if let Err(error) = device.set_control(Control {
        id: WHITE_BALANCE_AUTOMATIC,
        value: ControlValue::Boolean(true),
    }) {
        eprintln!("Warning: Failed to enable auto white balance: {error}");
    }

    // 防闪烁 - 设置为 50Hz (中国/欧洲)
    if let Err(error) = device.set_control(Control {
        id: POWER_LINE_FREQUENCY,
        value: ControlValue::Integer(1),
    }) {
        eprintln!("Warning: Failed to set anti-flicker (50Hz): {error}");
    }

    // 背光补偿 (等价于宽动态范围优化)
    if let Err(error) = device.set_control(Control {
        id: BACKLIGHT_COMPENSATION,
        value: ControlValue::Integer(2),
    }) {
        eprintln!("Warning: Failed to set backlight compensation: {error}");
    }

    // 自动曝光 - 光圈优先模式
    if let Err(error) = device.set_control(Control {
        id: AUTO_EXPOSURE,
        value: ControlValue::Integer(3),
    }) {
        eprintln!("Warning: Failed to set auto exposure (aperture priority): {error}");
    }

    // 启用动态帧率 (曝光优先)
    if let Err(error) = device.set_control(Control {
        id: EXPOSURE_DYNAMIC_FRAMERATE,
        value: ControlValue::Boolean(true),
    }) {
        eprintln!("Warning: Failed to enable exposure dynamic framerate: {error}");
    }

    Ok(())
}

fn video_device_paths() -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir("/dev") else {
        return Vec::new();
    };

    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_video_device_name)
        })
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| video_device_index(path).unwrap_or(u32::MAX));
    paths
}

fn is_video_device_name(name: &str) -> bool {
    name.strip_prefix("video")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()))
}

fn video_device_index(path: &Path) -> Option<u32> {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("video"))
        .and_then(|suffix| suffix.parse().ok())
}

fn select_format(device: &Device, request: &CameraRequest) -> Result<FrameFormat, String> {
    let formats = device
        .enum_formats()
        .map_err(|error| format!("Failed to enumerate V4L2 formats: {error}"))?;

    for preferred in &request.preferred_formats {
        if formats
            .iter()
            .any(|description| description.fourcc == preferred.fourcc())
        {
            return Ok(*preferred);
        }
    }

    Err(format!(
        "No supported V4L2 format found, wanted one of {:?}",
        request.preferred_formats
    ))
}

fn next_frame_before(stream: &mut MmapStream<'_>, timeout: Duration) -> Result<Vec<u8>, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match stream.next() {
            Ok((buffer, _)) => return Ok(buffer.to_vec()),
            Err(error) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
                if error.kind() == std::io::ErrorKind::WouldBlock {
                    continue;
                }
            }
            Err(error) => return Err(format!("Failed to read V4L2 frame: {error}")),
        }
    }
}

fn decode_frame(
    format: FrameFormat,
    width: u32,
    height: u32,
    stride: u32,
    data: &[u8],
) -> Result<RgbFrame, String> {
    match format {
        FrameFormat::Yuyv => decode_yuyv(width, height, stride, data),
        FrameFormat::Mjpeg => decode_mjpeg(data),
        FrameFormat::Nv12 => decode_nv12(width, height, stride, data),
        FrameFormat::Grey => decode_grey(width, height, stride, data),
    }
}

fn decode_grey(width: u32, height: u32, stride: u32, data: &[u8]) -> Result<RgbFrame, String> {
    let stride = stride.max(width) as usize;
    let width = width as usize;
    let height = height as usize;
    require_len("GREY", data, stride * height)?;

    let mut rgb = Vec::with_capacity(width * height * 3);
    for row in 0..height {
        let line = &data[row * stride..row * stride + width];
        for value in line {
            rgb.extend_from_slice(&[*value, *value, *value]);
        }
    }

    RgbFrame::new(width as u32, height as u32, rgb)
}

fn decode_yuyv(width: u32, height: u32, stride: u32, data: &[u8]) -> Result<RgbFrame, String> {
    let row_bytes = width as usize * 2;
    let stride = stride.max(row_bytes as u32) as usize;
    let width = width as usize;
    let height = height as usize;
    require_len("YUYV", data, stride * height)?;

    let mut rgb = Vec::with_capacity(width * height * 3);
    for row in 0..height {
        let line = &data[row * stride..row * stride + row_bytes];
        for chunk in line.chunks_exact(4) {
            let y0 = chunk[0];
            let u = chunk[1];
            let y1 = chunk[2];
            let v = chunk[3];
            rgb.extend_from_slice(&yuv_to_rgb(y0, u, v));
            rgb.extend_from_slice(&yuv_to_rgb(y1, u, v));
        }
    }

    RgbFrame::new(width as u32, height as u32, rgb)
}

fn decode_nv12(width: u32, height: u32, stride: u32, data: &[u8]) -> Result<RgbFrame, String> {
    let y_stride = stride.max(width) as usize;
    let uv_stride = y_stride;
    let width = width as usize;
    let height = height as usize;
    let y_size = y_stride * height;
    let uv_size = uv_stride * height.div_ceil(2);
    require_len("NV12", data, y_size + uv_size)?;

    let mut rgb = Vec::with_capacity(width * height * 3);
    for row in 0..height {
        let y_line = &data[row * y_stride..row * y_stride + width];
        let uv_row = y_size + (row / 2) * uv_stride;
        for (column, y) in y_line.iter().enumerate() {
            let uv_column = (column / 2) * 2;
            let u = data[uv_row + uv_column];
            let v = data[uv_row + uv_column + 1];
            rgb.extend_from_slice(&yuv_to_rgb(*y, u, v));
        }
    }

    RgbFrame::new(width as u32, height as u32, rgb)
}

fn decode_mjpeg(data: &[u8]) -> Result<RgbFrame, String> {
    let mut decoder = Decoder::new(Cursor::new(data));
    let decoded = decoder
        .decode()
        .map_err(|error| format!("Failed to decode MJPEG frame: {error}"))?;
    let info = decoder
        .info()
        .ok_or_else(|| "MJPEG frame did not include image metadata".to_string())?;

    let rgb = match info.pixel_format {
        JpegPixelFormat::RGB24 => decoded,
        JpegPixelFormat::L8 => decoded
            .iter()
            .flat_map(|value| [*value, *value, *value])
            .collect(),
        other => {
            return Err(format!(
                "Unsupported MJPEG decoded pixel format {:?}",
                other
            ))
        }
    };

    RgbFrame::new(info.width.into(), info.height.into(), rgb)
}

fn yuv_to_rgb(y: u8, u: u8, v: u8) -> [u8; 3] {
    let c = y as i32 - 16;
    let d = u as i32 - 128;
    let e = v as i32 - 128;

    [
        clamp_u8((298 * c + 409 * e + 128) >> 8),
        clamp_u8((298 * c - 100 * d - 208 * e + 128) >> 8),
        clamp_u8((298 * c + 516 * d + 128) >> 8),
    ]
}

fn clamp_u8(value: i32) -> u8 {
    value.clamp(0, 255) as u8
}

fn require_len(label: &str, data: &[u8], expected: usize) -> Result<(), String> {
    if data.len() < expected {
        Err(format!(
            "{label} frame too short: expected at least {expected} bytes, got {}",
            data.len()
        ))
    } else {
        Ok(())
    }
}

fn unsupported_format_message(fourcc: FourCC) -> String {
    format!(
        "V4L2 device returned unsupported format {}",
        fourcc.str().unwrap_or("<invalid>")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_device_name_parser_accepts_numbered_devices_only() {
        assert!(is_video_device_name("video0"));
        assert!(is_video_device_name("video12"));
        assert!(!is_video_device_name("video"));
        assert!(!is_video_device_name("video-control"));
    }

    #[test]
    fn missing_explicit_camera_is_unavailable() {
        let request = CameraRequest {
            device_path: Some(PathBuf::from("/dev/biopass-missing-camera")),
            ..CameraRequest::default()
        };

        assert!(!camera_available(&request));
    }

    #[test]
    fn grey_frame_expands_to_rgb() {
        let frame = decode_grey(2, 1, 2, &[0, 255]).unwrap();

        assert_eq!(frame.data, [0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn yuyv_frame_expands_pairs_to_rgb() {
        let frame = decode_yuyv(2, 1, 4, &[16, 128, 235, 128]).unwrap();

        assert_eq!(frame.data, [0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn nv12_frame_uses_shared_uv_samples() {
        let data = [16, 235, 81, 145, 128, 128, 128, 128];
        let frame = decode_nv12(2, 2, 2, &data).unwrap();

        assert_eq!(
            frame.data,
            [0, 0, 0, 255, 255, 255, 76, 76, 76, 150, 150, 150]
        );
    }

    #[test]
    fn rejects_short_frames() {
        let error = decode_yuyv(2, 1, 4, &[16, 128]).unwrap_err();

        assert!(error.contains("YUYV frame too short"));
    }

    #[test]
    fn grey_stats_compute_mean_min_max() {
        let data = [10, 20, 30, 40];
        let stats = calculate_grey_frame_stats(&data, 4, 1, 4);

        assert_eq!(stats.min, 10);
        assert_eq!(stats.max, 40);
        assert!((stats.mean - 25.0).abs() < 1e-9);
    }

    #[test]
    fn grey_stats_handle_undersized_buffers() {
        let stats = calculate_grey_frame_stats(&[0, 0, 0], 4, 1, 4);

        assert_eq!(stats, GreyFrameStats::default());
    }

    #[test]
    fn dark_ir_frame_requires_both_mean_and_max_below_thresholds() {
        let dark = GreyFrameStats {
            mean: 5.0,
            min: 0,
            max: 60,
        };
        let bright_max = GreyFrameStats {
            mean: 5.0,
            min: 0,
            max: 120,
        };
        let bright_mean = GreyFrameStats {
            mean: 50.0,
            min: 0,
            max: 60,
        };

        assert!(is_dark_ir_frame(dark));
        assert!(!is_dark_ir_frame(bright_max));
        assert!(!is_dark_ir_frame(bright_mean));
    }
}
