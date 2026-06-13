use super::{CameraRequest, FrameFormat};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use v4l::prelude::*;
use v4l::video::Capture;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VideoDevice {
    pub path: PathBuf,
    pub driver: String,
    pub card: String,
}

impl VideoDevice {
    pub fn display_name(&self) -> String {
        if self.card.is_empty() {
            self.path.display().to_string()
        } else {
            format!("{} ({})", self.card, self.path.display())
        }
    }

    pub fn path_str(&self) -> String {
        self.path.display().to_string()
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

pub(super) fn open_device(request: &CameraRequest) -> Result<Device, String> {
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

pub(super) fn select_format(
    device: &Device,
    request: &CameraRequest,
) -> Result<FrameFormat, String> {
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
}
