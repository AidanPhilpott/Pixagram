//! RAF decode + preview/thumbnail generation.
//!
//! Constraint #2 from the brief: X-Trans-aware decode. The actual color develop
//! lives in [`crate::develop`] — an owned, camera-general pipeline that reads the
//! raw mosaic + metadata and demosaics/white-balances/matrixes it ourselves,
//! because rawler 0.7.2's demosaic distorts X-Trans channel balance (see that
//! module and docs/INSIGHTS.md). This module is the thin I/O layer around it:
//! sidecar discovery, downscaled-image encoding, and `data:` URL packaging.
//!
//! Performance (brief's architecture rules):
//! - Thumbnails come from the camera JPEG sidecar when present (the user shoots
//!   RAW+JPEG), falling back to a downscaled owned develop. Browsing never decodes
//!   a full RAF when a JPEG is available.
//! - The editing preview decodes the RAF lazily and is downscaled *inside* the
//!   develop (the demosaic emits the reduced image directly), so a full 40MP
//!   buffer never crosses the IPC boundary.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::Engine;
use image::{DynamicImage, ImageFormat};

use crate::develop;

/// Longest-edge cap for the editing preview, in pixels. Big enough to judge
/// color and detail, small enough to stay responsive. Tunable, not architecture.
const PREVIEW_MAX_EDGE: u32 = 2048;

/// Longest-edge cap for grid thumbnails, in pixels.
const THUMB_MAX_EDGE: u32 = 384;

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
///
/// The develop is downscaled to [`PREVIEW_MAX_EDGE`] inside the owned pipeline
/// (the area-averaging demosaic produces the reduced image directly), so the
/// buffer never crosses IPC at full sensor resolution.
pub fn preview(path: &str) -> Result<PreviewPayload> {
    let developed = develop::develop_raf(Path::new(path), PREVIEW_MAX_EDGE)?;
    let (width, height) = (developed.width(), developed.height());
    let data_url = to_data_url(&developed, ImageFormat::Png)?;
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
            .with_context(|| format!("opening sidecar JPEG {}", jpg.display()))?
            .thumbnail(THUMB_MAX_EDGE, THUMB_MAX_EDGE),
        // No sidecar: develop the raw straight to thumbnail size via the owned path.
        None => develop::develop_raf(raf, THUMB_MAX_EDGE)?,
    };
    // JPEG keeps the thumbnail payload small; thumbnails are not edited.
    to_data_url(&source, ImageFormat::Jpeg)
}
