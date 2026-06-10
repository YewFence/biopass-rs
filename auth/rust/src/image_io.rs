use crate::RgbFrame;
use jpeg_decoder::{Decoder, PixelFormat as JpegPixelFormat};
use jpeg_encoder::{ColorType, Encoder};
use std::io::Cursor;

pub fn encode_jpeg(frame: &RgbFrame, quality: u8) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    Encoder::new(&mut output, quality)
        .encode(
            &frame.data,
            frame.width as u16,
            frame.height as u16,
            ColorType::Rgb,
        )
        .map_err(|error| format!("Failed to encode JPEG frame: {error}"))?;
    Ok(output)
}

pub fn decode_jpeg_rgb(data: &[u8]) -> Result<RgbFrame, String> {
    let mut decoder = Decoder::new(Cursor::new(data));
    let decoded = decoder
        .decode()
        .map_err(|error| format!("Failed to decode JPEG image: {error}"))?;
    let info = decoder
        .info()
        .ok_or_else(|| "JPEG image did not include metadata".to_string())?;

    let rgb = match info.pixel_format {
        JpegPixelFormat::RGB24 => decoded,
        JpegPixelFormat::L8 => decoded
            .iter()
            .flat_map(|value| [*value, *value, *value])
            .collect(),
        other => return Err(format!("Unsupported JPEG decoded pixel format {:?}", other)),
    };

    RgbFrame::new(info.width.into(), info.height.into(), rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_rgb_frame_as_jpeg() {
        let frame = RgbFrame::new(1, 1, vec![255, 0, 0]).unwrap();
        let jpeg = encode_jpeg(&frame, 80).unwrap();

        assert!(jpeg.starts_with(&[0xff, 0xd8]));
        assert!(jpeg.ends_with(&[0xff, 0xd9]));
    }

    #[test]
    fn decodes_jpeg_as_rgb_frame() {
        let frame = RgbFrame::new(1, 1, vec![255, 0, 0]).unwrap();
        let jpeg = encode_jpeg(&frame, 95).unwrap();

        let decoded = decode_jpeg_rgb(&jpeg).unwrap();

        assert_eq!(decoded.width, 1);
        assert_eq!(decoded.height, 1);
        assert_eq!(decoded.data.len(), 3);
    }
}
