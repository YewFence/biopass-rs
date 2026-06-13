use super::{FrameFormat, RgbFrame};
use jpeg_decoder::{Decoder, PixelFormat as JpegPixelFormat};
use std::io::Cursor;
use v4l::FourCC;

pub(super) fn decode_frame(
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

pub(super) fn decode_grey(
    width: u32,
    height: u32,
    stride: u32,
    data: &[u8],
) -> Result<RgbFrame, String> {
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
            ));
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

pub(super) fn unsupported_format_message(fourcc: FourCC) -> String {
    format!(
        "V4L2 device returned unsupported format {}",
        fourcc.str().unwrap_or("<invalid>")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
