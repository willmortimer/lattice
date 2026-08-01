//! Still-capture rendition helpers (ADR 0053 tier 0).
//!
//! PNG clipboard and lossless WebP storage encoding live here so native bridges
//! and desktop hosts do not duplicate format policy.

use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder};

use crate::image::{CapturedImage, ImageData};

/// Normalize backend pixels to PNG bytes for clipboard renditions.
pub fn png_bytes_from_capture(captured: &CapturedImage) -> Result<Vec<u8>, String> {
    match &captured.data {
        ImageData::Png(bytes) => Ok(bytes.clone()),
        ImageData::Rgba { bytes, .. } => {
            let mut buf = Vec::new();
            let encoder = image::codecs::png::PngEncoder::new(&mut buf);
            encoder
                .write_image(
                    bytes,
                    captured.width,
                    captured.height,
                    ExtendedColorType::Rgba8,
                )
                .map_err(|err| format!("failed to encode capture PNG: {err}"))?;
            Ok(buf)
        }
    }
}

/// Encode PNG bytes for Capture Inbox storage (lossless WebP, PNG fallback).
pub fn encode_storage_image(png_bytes: &[u8]) -> Result<(String, Vec<u8>), String> {
    let image = image::load_from_memory(png_bytes)
        .map_err(|err| format!("failed to decode capture image: {err}"))?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();

    let mut webp = Vec::new();
    let encoder = WebPEncoder::new_lossless(&mut webp);
    match encoder.write_image(rgba.as_raw(), width, height, ExtendedColorType::Rgba8) {
        Ok(()) => Ok(("capture.webp".to_string(), webp)),
        Err(err) => {
            eprintln!("lattice: WebP encode failed, storing PNG: {err}");
            Ok(("capture.png".to_string(), png_bytes.to_vec()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_variant_round_trips() {
        let bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let captured = CapturedImage::png(1, 1, bytes.clone());
        assert_eq!(png_bytes_from_capture(&captured).unwrap(), bytes);
    }
}
