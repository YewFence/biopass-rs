use v4l::FourCC;

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
