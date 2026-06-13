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
