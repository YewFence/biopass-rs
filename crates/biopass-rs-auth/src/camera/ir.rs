use super::decode::decode_grey;
use super::stream::next_frame_before;
use super::RgbFrame;
use crate::{emit_log, LogLevel};
use std::time::Instant;
use v4l::io::mmap::Stream as MmapStream;

const DARK_IR_MEAN_THRESHOLD: f64 = 10.0;
const DARK_IR_MAX_THRESHOLD: u8 = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GreyFrameLayout {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) stride: u32,
}

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

/// Captures an IR frame from a V4L2 GREY stream, skipping dark frames until
/// a sufficiently bright one is found. On timeout or max_dark_frames limit,
/// returns the last dark frame instead of failing completely.
pub(super) fn capture_grey_ir_frame(
    stream: &mut MmapStream<'_>,
    warmup: &[u8],
    layout: GreyFrameLayout,
    deadline: &Instant,
    max_dark_frames: u32,
    debug: bool,
) -> Result<RgbFrame, String> {
    let GreyFrameLayout {
        width,
        height,
        stride,
    } = layout;

    let mut skipped_dark_frames: u32 = 0;
    let mut last_dark: Option<(GreyFrameStats, RgbFrame)>;

    let (stats, dark) = grey_frame_stats_and_dark(warmup, width, height, stride);
    if !dark {
        return decode_grey(width, height, stride, warmup);
    }
    skipped_dark_frames += 1;
    last_dark = Some((stats, decode_grey(width, height, stride, warmup)?));
    emit_log(
        LogLevel::Debug,
        debug,
        "camera:ir",
        &format!(
            "skipping dark IR frame from V4L2 GREY mean={:.2}, min={}, max={}, skipped={}",
            stats.mean, stats.min, stats.max, skipped_dark_frames
        ),
    );

    loop {
        if skipped_dark_frames >= max_dark_frames {
            if let Some((stats, frame)) = last_dark.take() {
                emit_log(
                    LogLevel::Warn,
                    debug,
                    "camera:ir",
                    &format!(
                        "reached max dark frames limit ({}) for V4L2 GREY, \
                         returning last dark frame mean={:.2}, min={}, max={}",
                        max_dark_frames, stats.mean, stats.min, stats.max
                    ),
                );
                return Ok(frame);
            }
        }

        if Instant::now() >= *deadline {
            if let Some((stats, frame)) = last_dark.take() {
                emit_log(
                    LogLevel::Warn,
                    debug,
                    "camera:ir",
                    &format!(
                        "timed out waiting for non-dark V4L2 GREY frame after skipping \
                         {} dark frame(s), returning last dark frame mean={:.2}, min={}, max={}",
                        skipped_dark_frames, stats.mean, stats.min, stats.max
                    ),
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
            emit_log(
                LogLevel::Debug,
                debug,
                "camera:ir",
                &format!(
                    "skipping dark IR frame from V4L2 GREY mean={:.2}, min={}, max={}, skipped={}",
                    stats.mean, stats.min, stats.max, skipped_dark_frames
                ),
            );
            continue;
        }

        emit_log(
            LogLevel::Debug,
            debug,
            "camera:ir",
            &format!(
                "returning V4L2 GREY IR frame mean={:.2}, min={}, max={}, skipped_dark={}",
                stats.mean, stats.min, stats.max, skipped_dark_frames
            ),
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

#[cfg(test)]
mod tests {
    use super::*;

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
