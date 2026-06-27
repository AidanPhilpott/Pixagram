//! RAF decode + preview/thumbnail generation.
//!
//! Constraint #2 from the brief: X-Trans-aware decode. We lean on `rawler`'s
//! `RawDevelop`, which dispatches the correct demosaic (Markesteijn for the
//! X-T5's X-Trans CFA) and outputs an sRGB image — so exported/previewed JPEGs
//! are correctly sRGB-tagged downstream.
//!
//! Performance (brief's architecture rules):
//! - Thumbnails come from the camera JPEG sidecar when present (the user shoots
//!   RAW+JPEG), falling back to a downscaled RAF develop. Browsing never decodes
//!   a full RAF when a JPEG is available.
//! - The editing preview decodes the RAF lazily and is downscaled before it
//!   crosses the IPC boundary; we never ship a full 40MP buffer to the frontend.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use image::{DynamicImage, ImageFormat};

/// Longest-edge cap for the editing preview, in pixels. Big enough to judge
/// color and detail, small enough to stay responsive. Tunable, not architecture.
const PREVIEW_MAX_EDGE: u32 = 2048;

/// Longest-edge cap for grid thumbnails, in pixels.
const THUMB_MAX_EDGE: u32 = 384;

/// Decode a Fujifilm RAF and develop it to an sRGB [`DynamicImage`].
///
/// This is the only place that pays the full X-Trans demosaic cost. We also
/// apply the camera's as-shot white balance here (see [`apply_as_shot_wb`]):
/// that is part of *developing the raw to a neutral image*, not a user edit —
/// the Phase 2 temp/tint slider will adjust relative to this baseline.
pub fn develop_raf<P: AsRef<Path>>(path: P) -> Result<DynamicImage> {
    let path = path.as_ref();
    let raw = rawler::decode_file(path)
        .map_err(|e| anyhow!("RAF decode failed for {}: {e}", path.display()))?;

    let developer = rawler::imgop::develop::RawDevelop::default();
    let intermediate = developer
        .develop_intermediate(&raw)
        .map_err(|e| anyhow!("develop failed for {}: {e}", path.display()))?;

    let developed = intermediate
        .to_dynamic_image()
        .ok_or_else(|| anyhow!("develop produced no displayable image for {}", path.display()))?;

    Ok(apply_as_shot_wb(developed, raw.wb_coeffs))
}

/// Apply the camera's as-shot white balance to a developed sRGB image.
///
/// **Why this exists:** rawler 0.7.2's `RawDevelop` does not effectively apply
/// the as-shot WB for X-T5 files — its output comes out green-dominant, and one
/// application of `wb_coeffs` neutralizes it (verified on three diverse real
/// X-Trans frames: neutral subjects go neutral, real greens stay green). We
/// apply the coefficients as a direct gain on the encoded sRGB values,
/// normalized so green = 1.0 (a pure WB, no net green-channel change).
///
/// Applied post-develop (after rawler's color matrix) it's an empirical
/// correction rather than the colorimetrically ideal "WB in camera-linear space
/// before the matrix" — but it matches correct color closely across test frames
/// and is what we can do without reimplementing rawler's calibration.
/// `wb_coeffs[3]` (the X-Trans-absent second green) is NaN and ignored. Revisit
/// if a future rawler bump fixes WB internally, or when Phase 2 builds the WB
/// adjustment and may take over the color pipeline.
fn apply_as_shot_wb(img: DynamicImage, wb: [f32; 4]) -> DynamicImage {
    let g = if wb[1].is_finite() && wb[1] != 0.0 { wb[1] } else { 1.0 };
    let gain = [
        if wb[0].is_finite() { wb[0] / g } else { 1.0 },
        1.0,
        if wb[2].is_finite() { wb[2] / g } else { 1.0 },
    ];

    // Nothing to do if the camera reported a neutral (or missing) WB.
    if (gain[0] - 1.0).abs() < 1e-3 && (gain[2] - 1.0).abs() < 1e-3 {
        return img;
    }

    let mut rgb = img.into_rgb16();
    for px in rgb.pixels_mut() {
        px[0] = (px[0] as f32 * gain[0]).min(65535.0) as u16;
        px[2] = (px[2] as f32 * gain[2]).min(65535.0) as u16;
    }
    DynamicImage::ImageRgb16(rgb)
}

/// Find a camera-JPEG sidecar (`.JPG`/`.JPEG`, any case) next to a RAF.
fn sidecar_jpeg(raf: &Path) -> Option<PathBuf> {
    let stem = raf.file_stem()?;
    let dir = raf.parent()?;
    for ext in ["JPG", "jpg", "JPEG", "jpeg"] {
        let candidate = dir.join(format!("{}.{}", stem.to_string_lossy(), ext));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Encode a [`DynamicImage`] to a `data:` URL (base64).
fn to_data_url(img: &DynamicImage, format: ImageFormat) -> Result<String> {
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut Cursor::new(&mut bytes), format)
        .context("encoding preview image")?;
    let mime = match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        _ => "application/octet-stream",
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

/// A downscaled, sRGB editing preview ready for GPU upload.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPayload {
    pub width: u32,
    pub height: u32,
    /// PNG `data:` URL. The frontend uploads this to a WebGL texture; exposure
    /// and later adjustments are applied in-shader from the shared definition.
    pub data_url: String,
}

/// Decode + develop a RAF and return a downscaled PNG preview.
pub fn preview(path: &str) -> Result<PreviewPayload> {
    let developed = develop_raf(path)?;
    // Downscale before crossing IPC — never ship a full 40MP buffer.
    let scaled = developed.resize(
        PREVIEW_MAX_EDGE,
        PREVIEW_MAX_EDGE,
        image::imageops::FilterType::Lanczos3,
    );
    let (width, height) = (scaled.width(), scaled.height());
    let data_url = to_data_url(&scaled, ImageFormat::Png)?;
    Ok(PreviewPayload {
        width,
        height,
        data_url,
    })
}

/// Produce a small JPEG thumbnail `data:` URL for the grid.
///
/// Prefers the camera-JPEG sidecar for speed; falls back to a downscaled RAF
/// develop when there is no sidecar.
pub fn thumbnail(raf_path: &str) -> Result<String> {
    let raf = Path::new(raf_path);
    let source = match sidecar_jpeg(raf) {
        Some(jpg) => image::open(&jpg)
            .with_context(|| format!("opening sidecar JPEG {}", jpg.display()))?,
        None => develop_raf(raf)?,
    };
    let thumb = source.thumbnail(THUMB_MAX_EDGE, THUMB_MAX_EDGE);
    // JPEG keeps the thumbnail payload small; thumbnails are not edited.
    to_data_url(&thumb, ImageFormat::Jpeg)
}
