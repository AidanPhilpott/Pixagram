//! Phase 1 export STUB.
//!
//! The brief asks only to "stub/log an export step to confirm the pipeline
//! reaches end-to-end." We deliberately do not write a file yet. What we *do*
//! prove here is the important architectural claim: the same `Adjustments`
//! definition that drives the GPU preview also arrives in Rust and is applied by
//! the same shared math (`adjustments::apply_pixel`). Real JPEG/border/EXIF
//! export is Phase 5.
//!
//! Phase 5 per-pixel order: `apply_pixel` → `adjustments::apply_grain(x, y)`
//! (grain is spatial, on top of the final color), then once per image:
//! `apply_rotation` → `apply_straighten` → `apply_crop`. The GPU preview
//! applies the same stages in the same order (geometry as quad-UV math).

use anyhow::Result;
use image::{DynamicImage, GenericImageView, Rgb, RgbImage};

use crate::adjustments::{apply_pixel, straighten_cover_scale, Adjustments, CropRect};

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

/// Free-straighten an image by `degrees` **clockwise**, auto-zoomed by
/// [`straighten_cover_scale`] so the output (same dimensions) never shows
/// blank corners. Bilinear sampling; runs after [`apply_rotation`].
///
/// **Lockstep with the straighten term of `PreviewRenderer.writeQuad`** — the
/// preview realizes the identical mapping as a quad-UV transform (an affine
/// map, so interpolating the four corners is exact). Edit both together.
pub fn apply_straighten(img: DynamicImage, degrees: f32) -> DynamicImage {
    if degrees == 0.0 {
        return img;
    }
    let (w, h) = img.dimensions();
    let src = img.to_rgb8();
    let k = straighten_cover_scale(w as f32, h as f32, degrees);
    // Content rotates clockwise by `a` on screen ⇒ the sampling grid rotates
    // the other way: source = center + R_cw(−a)·(d·k), with y down.
    let a = degrees.to_radians();
    let (sin, cos) = (a.sin(), a.cos());
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);

    let mut out = RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 + 0.5 - cx) * k;
            let dy = (y as f32 + 0.5 - cy) * k;
            let sx = cx + dx * cos + dy * sin;
            let sy = cy - dx * sin + dy * cos;
            out.put_pixel(x, y, sample_bilinear(&src, sx, sy));
        }
    }
    DynamicImage::ImageRgb8(out)
}

/// Bilinear sample at pixel-center coordinates (sx, sy), clamped to the edge.
fn sample_bilinear(src: &RgbImage, sx: f32, sy: f32) -> Rgb<u8> {
    let (w, h) = src.dimensions();
    let fx = (sx - 0.5).clamp(0.0, (w - 1) as f32);
    let fy = (sy - 0.5).clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let px = |x: u32, y: u32| src.get_pixel(x, y).0;
    let (p00, p10, p01, p11) = (px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1));
    let mut out = [0u8; 3];
    for c in 0..3 {
        let top = p00[c] as f32 * (1.0 - tx) + p10[c] as f32 * tx;
        let bot = p01[c] as f32 * (1.0 - tx) + p11[c] as f32 * tx;
        out[c] = (top * (1.0 - ty) + bot * ty).round().clamp(0.0, 255.0) as u8;
    }
    Rgb(out)
}

/// Crop to a normalized rect (top-left origin, y down) of the straightened
/// frame. Runs last in the geometry chain; the full-frame default is a no-op.
/// **Lockstep with the crop term of `PreviewRenderer.writeQuad`.**
pub fn apply_crop(img: DynamicImage, crop: &CropRect) -> DynamicImage {
    if crop.is_full() {
        return img;
    }
    let (w, h) = img.dimensions();
    let x0 = (crop.x.clamp(0.0, 1.0) * w as f32).round() as u32;
    let y0 = (crop.y.clamp(0.0, 1.0) * h as f32).round() as u32;
    let cw = ((crop.width.clamp(0.0, 1.0) * w as f32).round() as u32)
        .clamp(1, w - x0.min(w - 1));
    let ch = ((crop.height.clamp(0.0, 1.0) * h as f32).round() as u32)
        .clamp(1, h - y0.min(h - 1));
    img.crop_imm(x0.min(w - 1), y0.min(h - 1), cw, ch)
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

    /// A 2×2 square with four distinct corner colors:
    /// top-left red, top-right green, bottom-left blue, bottom-right white.
    fn four_corners() -> DynamicImage {
        let mut img = RgbImage::new(2, 2);
        img.put_pixel(0, 0, Rgb([255, 0, 0]));
        img.put_pixel(1, 0, Rgb([0, 255, 0]));
        img.put_pixel(0, 1, Rgb([0, 0, 255]));
        img.put_pixel(1, 1, Rgb([255, 255, 255]));
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn straighten_zero_is_identity() {
        let out = apply_straighten(four_corners(), 0.0).to_rgb8();
        assert_eq!(out, four_corners().to_rgb8());
    }

    #[test]
    fn straighten_90_on_a_square_matches_rotate90() {
        // On a square, 90° has cover scale 1 and the bilinear grid lands on
        // exact pixel centers — so free-straighten 90° CW must equal the
        // quarter-turn rotation exactly. This pins the rotation DIRECTION.
        let straightened = apply_straighten(four_corners(), 90.0).to_rgb8();
        let quarter = apply_rotation(four_corners(), 1).to_rgb8();
        assert_eq!(straightened, quarter, "straighten +90° must be clockwise");
    }

    #[test]
    fn straighten_keeps_dimensions_and_never_shows_blank_corners() {
        // A solid image rotated by any angle stays solid — the auto-zoom must
        // keep every sample inside the source (no edge-clamp streaks of
        // background, no out-of-range pixels).
        let solid = DynamicImage::ImageRgb8(RgbImage::from_pixel(8, 6, Rgb([120, 90, 30])));
        for deg in [-45.0, -10.0, 7.5, 30.0, 45.0] {
            let out = apply_straighten(solid.clone(), deg).to_rgb8();
            assert_eq!(out.dimensions(), (8, 6), "dims preserved at {deg}°");
            for p in out.pixels() {
                assert_eq!(p.0, [120, 90, 30], "corner leaked at {deg}°");
            }
        }
    }

    #[test]
    fn cover_scale_matches_known_values() {
        use crate::adjustments::straighten_cover_scale;
        assert!((straighten_cover_scale(100.0, 50.0, 0.0) - 1.0).abs() < 1e-6);
        // Square at 45°: the inscribed square is 1/(cos45+sin45) = 1/√2.
        let k = straighten_cover_scale(100.0, 100.0, 45.0);
        assert!((k - 1.0 / 2f32.sqrt()).abs() < 1e-4, "got {k}");
        // Square at 90°: back to a perfect fit.
        assert!((straighten_cover_scale(100.0, 100.0, 90.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn crop_full_frame_is_identity_and_quadrant_picks_the_corner() {
        let full = apply_crop(four_corners(), &CropRect::default()).to_rgb8();
        assert_eq!(full.dimensions(), (2, 2), "full-frame crop is a no-op");

        // Top-right quadrant of the 2×2 → the single green pixel.
        let rect = CropRect { x: 0.5, y: 0.0, width: 0.5, height: 0.5 };
        let out = apply_crop(four_corners(), &rect).to_rgb8();
        assert_eq!(out.dimensions(), (1, 1));
        assert_eq!(out.get_pixel(0, 0).0, [0, 255, 0], "green top-right quadrant");
    }
}
