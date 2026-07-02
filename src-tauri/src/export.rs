//! Phase 1 export STUB.
//!
//! The brief asks only to "stub/log an export step to confirm the pipeline
//! reaches end-to-end." We deliberately do not write a file yet. What we *do*
//! prove here is the important architectural claim: the same `Adjustments`
//! definition that drives the GPU preview also arrives in Rust and is applied by
//! the same shared math (`adjustments::apply_pixel`). Real JPEG/border/EXIF
//! export is Phase 5.

use anyhow::Result;
use image::DynamicImage;

use crate::adjustments::{apply_pixel, Adjustments};

/// Apply the stack's rotation (clockwise quarter-turns) to a developed image.
///
/// Geometry, not color: this runs once per image **after** `apply_pixel`, never
/// per pixel. The GPU preview mirrors it by rotating the quad's texture
/// coordinates and swapping the canvas aspect (`applyGeometry` in
/// `src/gl/PreviewRenderer.ts`) — both sides rotate clockwise, and the real
/// Phase 5 exporter MUST call this after the per-pixel math so preview
/// orientation == export orientation.
pub fn apply_rotation(img: DynamicImage, rotation: u8) -> DynamicImage {
    match rotation % 4 {
        1 => img.rotate90(),
        2 => img.rotate180(),
        3 => img.rotate270(),
        _ => img,
    }
}

/// Simulate an export and return a human-readable summary for the UI.
pub fn export_stub(path: &str, adj: &Adjustments) -> Result<String> {
    // Demonstrate the shared definition is live on the Rust side by running a
    // sample pixel through the exact same math the shader uses for the preview.
    let sample_in = [0.5_f32, 0.5, 0.5];
    let sample_out = apply_pixel(adj, sample_in);

    log::info!(
        "[export stub] path={path} exposure={:+.2} EV rotation={}° | mid-grey {:.3} -> {:.3} (shared math)",
        adj.exposure,
        u32::from(adj.rotation % 4) * 90,
        sample_in[0],
        sample_out[0],
    );

    Ok(format!(
        "Export stub OK — pipeline reached end-to-end.\n\
         • Source: {path}\n\
         • Exposure: {:+.2} EV\n\
         • Rotation: {}° clockwise (applied via apply_rotation after pixel math)\n\
         • Shared math check: mid-grey 0.500 → {:.3}\n\
         (Real JPEG + border + sRGB + EXIF export lands in Phase 5.)",
        adj.exposure,
        u32::from(adj.rotation % 4) * 90,
        sample_out[0],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    /// A 2×1 image: red on the left, blue on the right.
    fn two_color() -> DynamicImage {
        let mut img = RgbImage::new(2, 1);
        img.put_pixel(0, 0, Rgb([255, 0, 0]));
        img.put_pixel(1, 0, Rgb([0, 0, 255]));
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn rotation_0_is_identity() {
        for rot in [0u8, 4] {
            let out = apply_rotation(two_color(), rot).to_rgb8();
            assert_eq!(out.dimensions(), (2, 1));
            assert_eq!(out.get_pixel(0, 0).0, [255, 0, 0], "red stays left (rot={rot})");
        }
    }

    #[test]
    fn rotation_90_is_clockwise() {
        // Rotating [red | blue] 90° clockwise: the left edge becomes the top,
        // so red lands on top of a 1×2 column.
        let out = apply_rotation(two_color(), 1).to_rgb8();
        assert_eq!(out.dimensions(), (1, 2), "90° swaps width/height");
        assert_eq!(out.get_pixel(0, 0).0, [255, 0, 0], "red on top");
        assert_eq!(out.get_pixel(0, 1).0, [0, 0, 255], "blue on the bottom");
    }

    #[test]
    fn rotation_180_mirrors_both_axes() {
        let out = apply_rotation(two_color(), 2).to_rgb8();
        assert_eq!(out.dimensions(), (2, 1));
        assert_eq!(out.get_pixel(0, 0).0, [0, 0, 255], "blue now left");
        assert_eq!(out.get_pixel(1, 0).0, [255, 0, 0], "red now right");
    }

    #[test]
    fn rotation_270_is_counter_clockwise() {
        // 270° CW == 90° CCW: the left edge becomes the bottom, so red lands
        // at the bottom of a 1×2 column.
        let out = apply_rotation(two_color(), 3).to_rgb8();
        assert_eq!(out.dimensions(), (1, 2));
        assert_eq!(out.get_pixel(0, 0).0, [0, 0, 255], "blue on top");
        assert_eq!(out.get_pixel(0, 1).0, [255, 0, 0], "red on the bottom");
    }
}
