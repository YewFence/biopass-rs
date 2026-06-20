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

    const LIMITED_RANGE_BLACK_Y: u8 = 16;
    const LIMITED_RANGE_WHITE_Y: u8 = 235;
    const NEUTRAL_CHROMA: u8 = 128;
    const ROW_PADDING: u8 = 0xee;

    fn assert_rgb_close(actual: &[u8], expected: &[u8]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert!((*actual as i16 - *expected as i16).abs() <= 1);
        }
    }

    #[test]
    fn grey_frame_expands_to_rgb() {
        let frame = decode_grey(2, 1, 2, &[0, 255]).unwrap();

        assert_eq!(frame.data, [0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn grey_frame_ignores_stride_padding() {
        let padded = decode_grey(2, 2, 3, &[10, 20, ROW_PADDING, 30, 40, ROW_PADDING]).unwrap();
        let compact = decode_grey(2, 2, 2, &[10, 20, 30, 40]).unwrap();

        assert_eq!(padded, compact);
    }

    #[test]
    fn yuyv_frame_expands_pairs_to_rgb() {
        let frame = decode_yuyv(
            2,
            1,
            4,
            &[
                LIMITED_RANGE_BLACK_Y,
                NEUTRAL_CHROMA,
                LIMITED_RANGE_WHITE_Y,
                NEUTRAL_CHROMA,
            ],
        )
        .unwrap();

        assert_rgb_close(&frame.data, &[0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn yuyv_frame_ignores_stride_padding() {
        let first_row = [
            LIMITED_RANGE_BLACK_Y,
            NEUTRAL_CHROMA,
            LIMITED_RANGE_WHITE_Y,
            NEUTRAL_CHROMA,
        ];
        let second_row = [81, 90, 145, 240];
        let padded = decode_yuyv(
            2,
            2,
            6,
            &[
                first_row[0],
                first_row[1],
                first_row[2],
                first_row[3],
                ROW_PADDING,
                ROW_PADDING,
                second_row[0],
                second_row[1],
                second_row[2],
                second_row[3],
                ROW_PADDING,
                ROW_PADDING,
            ],
        )
        .unwrap();
        let compact = decode_yuyv(2, 2, 4, &[first_row, second_row].concat()).unwrap();

        assert_eq!(padded, compact);
    }

    #[test]
    fn nv12_frame_uses_shared_uv_samples() {
        let data = [
            LIMITED_RANGE_BLACK_Y,
            LIMITED_RANGE_WHITE_Y,
            81,
            145,
            NEUTRAL_CHROMA,
            NEUTRAL_CHROMA,
            NEUTRAL_CHROMA,
            NEUTRAL_CHROMA,
        ];
        let frame = decode_nv12(2, 2, 2, &data).unwrap();

        assert_rgb_close(
            &frame.data,
            &[0, 0, 0, 255, 255, 255, 76, 76, 76, 150, 150, 150],
        );
    }

    #[test]
    fn nv12_frame_handles_odd_height_and_stride_padding() {
        let y_plane_with_padding = [
            LIMITED_RANGE_BLACK_Y,
            LIMITED_RANGE_WHITE_Y,
            ROW_PADDING,
            81,
            145,
            ROW_PADDING,
            90,
            240,
            ROW_PADDING,
        ];
        let uv_plane_with_padding = [
            NEUTRAL_CHROMA,
            NEUTRAL_CHROMA,
            ROW_PADDING,
            54,
            34,
            ROW_PADDING,
        ];
        let mut padded_data = y_plane_with_padding.to_vec();
        padded_data.extend_from_slice(&uv_plane_with_padding);
        let padded = decode_nv12(2, 3, 3, &padded_data).unwrap();
        let compact = decode_nv12(
            2,
            3,
            2,
            &[
                LIMITED_RANGE_BLACK_Y,
                LIMITED_RANGE_WHITE_Y,
                81,
                145,
                90,
                240,
                NEUTRAL_CHROMA,
                NEUTRAL_CHROMA,
                54,
                34,
            ],
        )
        .unwrap();

        assert_eq!(padded, compact);
    }

    #[test]
    fn decode_frame_dispatches_by_format() {
        let frame = decode_frame(FrameFormat::Grey, 1, 1, 1, &[7]).unwrap();

        assert_eq!(frame.data, [7, 7, 7]);
    }

    #[test]
    fn rejects_short_frames() {
        let error = decode_yuyv(2, 1, 4, &[16, 128]).unwrap_err();

        assert!(error.contains("YUYV frame too short"));
    }

    #[test]
    fn unsupported_format_message_handles_invalid_fourcc() {
        let message = unsupported_format_message(FourCC { repr: [0xff; 4] });

        assert!(message.contains("<invalid>"));
    }
}
