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
/// This is the only place that pays the full X-Trans demosaic cost. rawler's
/// `RawDevelop` already applies the as-shot white balance and camera color matrix
/// and outputs sRGB. (Do NOT re-apply white balance here — rawler's WhiteBalance
/// step is active; a second pass double-corrects and pushes highlights magenta.
/// A milder green/cyan residual remains in rawler 0.7.2's X-T5 color — that is a
/// matrix/illuminant issue to address properly in Phase 2's color work, not with
/// a global gain. See docs/INSIGHTS.md.)
pub fn develop_raf<P: AsRef<Path>>(path: P) -> Result<DynamicImage> {
    let path = path.as_ref();
    let raw = rawler::decode_file(path)
        .map_err(|e| anyhow!("RAF decode failed for {}: {e}", path.display()))?;

    let developer = rawler::imgop::develop::RawDevelop::default();
    let intermediate = developer
        .develop_intermediate(&raw)
        .map_err(|e| anyhow!("develop failed for {}: {e}", path.display()))?;

    intermediate
        .to_dynamic_image()
        .ok_or_else(|| anyhow!("develop produced no displayable image for {}", path.display()))
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
