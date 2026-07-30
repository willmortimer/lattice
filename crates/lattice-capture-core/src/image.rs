//! Captured pixel payload returned by backends.

/// PNG or raw RGBA bytes plus dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    pub data: ImageData,
}

/// Encoded or raw pixel bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageData {
    Png(Vec<u8>),
    Rgba { bytes: Vec<u8>, mime: String },
}

impl CapturedImage {
    pub fn png(width: u32, height: u32, png_bytes: Vec<u8>) -> Self {
        Self {
            width,
            height,
            data: ImageData::Png(png_bytes),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_constructor_wraps_bytes() {
        let bytes = vec![0x89, 0x50, 0x4E, 0x47];
        let image = CapturedImage::png(800, 600, bytes.clone());
        assert_eq!(image.width, 800);
        assert_eq!(image.height, 600);
        assert_eq!(image.data, ImageData::Png(bytes));
    }

    #[test]
    fn rgba_variant_preserves_mime() {
        let image = CapturedImage {
            width: 10,
            height: 10,
            data: ImageData::Rgba {
                bytes: vec![0; 400],
                mime: "image/rgba".into(),
            },
        };
        match image.data {
            ImageData::Rgba { mime, .. } => assert_eq!(mime, "image/rgba"),
            ImageData::Png(_) => panic!("expected rgba variant"),
        }
    }
}
