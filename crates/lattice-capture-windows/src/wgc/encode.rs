//! Encode captured RGBA pixels as PNG.

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use lattice_capture_core::{CaptureError, CapturedImage};

pub fn rgba_to_png_image(width: u32, height: u32, rgba: &[u8]) -> Result<CapturedImage, CaptureError> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() != expected {
        return Err(CaptureError::internal(format!(
            "RGBA buffer length {} != expected {expected} ({width}x{height})",
            rgba.len()
        )));
    }
    let mut png = Vec::new();
    {
        let encoder = PngEncoder::new(&mut png);
        encoder
            .write_image(rgba, width, height, ExtendedColorType::Rgba8)
            .map_err(|err| CaptureError::internal(format!("PNG encode failed: {err}")))?;
    }
    if png.is_empty() {
        return Err(CaptureError::internal("PNG encode produced empty output"));
    }
    Ok(CapturedImage::png(width, height, png))
}

/// Crop an RGBA buffer in display pixel space.
pub fn crop_rgba(
    src_width: u32,
    src_height: u32,
    rgba: &[u8],
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, CaptureError> {
    if width == 0 || height == 0 {
        return Err(CaptureError::invalid_argument("region size must be non-zero"));
    }
    if x.saturating_add(width) > src_width || y.saturating_add(height) > src_height {
        return Err(CaptureError::invalid_argument(format!(
            "region {x},{y} {width}x{height} exceeds capture {src_width}x{src_height}"
        )));
    }
    let mut out = vec![0u8; width as usize * height as usize * 4];
    for row in 0..height as usize {
        let src_off = ((y as usize + row) * src_width as usize + x as usize) * 4;
        let dst_off = row * width as usize * 4;
        out[dst_off..dst_off + width as usize * 4]
            .copy_from_slice(&rgba[src_off..src_off + width as usize * 4]);
    }
    Ok(out)
}
