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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_exact_rgb_payload_size() {
        let frame = RgbFrame::new(2, 1, vec![1, 2, 3, 4, 5, 6]).unwrap();

        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 1);
        assert_eq!(frame.data, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn rejects_short_rgb_payload_size() {
        let error = RgbFrame::new(2, 1, vec![1, 2, 3]).unwrap_err();

        assert_eq!(error, "RGB frame size mismatch: expected 6 bytes, got 3");
    }
}
