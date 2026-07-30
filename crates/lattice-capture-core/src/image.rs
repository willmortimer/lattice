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
