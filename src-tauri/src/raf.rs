//! Raw decode + preview/thumbnail generation. (Named `raf` for the Fujifilm-RAF
//! origin, but format-generic: any raw rawler decodes — NEF, CR2/CR3, ARW, DNG,
//! ORF, RW2, PEF, … — flows through the same paths.)
//!
//! Constraint #2 from the brief: X-Trans-aware decode. The actual color develop
//! lives in [`crate::develop`] — an owned, camera-general pipeline that reads the
//! raw mosaic + metadata and demosaics/white-balances/matrixes it ourselves,
//! because rawler 0.7.2's demosaic distorts X-Trans channel balance (see that
//! module and docs/INSIGHTS.md). This module is the thin I/O layer around it:
//! sidecar/embedded-JPEG discovery, downscaled-image encoding, and `data:` URL
//! packaging.
//!
//! Performance (brief's architecture rules):
//! - Thumbnails never develop the raw if a camera JPEG exists: sidecar `.JPG`
//!   first (Fuji RAW+JPEG shooters), then the preview JPEG embedded in the raw
//!   itself (nearly every format carries one), and only then a downscaled develop.
//! - The editing preview decodes the raw lazily and is downscaled *inside* the
//!   develop (the demosaic emits the reduced image directly), so a full 40MP
//!   buffer never crosses the IPC boundary.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::Engine;
use image::{DynamicImage, ImageFormat};
use rawler::decoders::RawDecodeParams;
use rawler::rawsource::RawSource;

use crate::develop;

/// Longest-edge cap for the editing preview, in pixels. Big enough to judge
/// color and detail, small enough to stay responsive. Tunable, not architecture.
const PREVIEW_MAX_EDGE: u32 = 2048;

/// Longest-edge cap for grid thumbnails, in pixels.
const THUMB_MAX_EDGE: u32 = 384;

/// Find a camera-JPEG sidecar (`.JPG`/`.JPEG`, any case) next to a raw file.
fn sidecar_jpeg(raw: &Path) -> Option<PathBuf> {
    let stem = raw.file_stem()?;
    let dir = raw.parent()?;
    for ext in ["JPG", "jpg", "JPEG", "jpeg"] {
        let candidate = dir.join(format!("{}.{}", stem.to_string_lossy(), ext));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Extract the preview JPEG embedded in the raw file itself, if the format's
/// decoder exposes one. Non-Fuji cameras rarely ship a sidecar, but nearly every
/// raw format embeds a camera-rendered JPEG — decoding it is far cheaper than a
/// raw develop, so this is the thumbnail path of choice after the sidecar.
/// Tries the decoder's images smallest-first (thumbnail → preview → full).
fn embedded_jpeg(raw: &Path) -> Option<DynamicImage> {
    let source = RawSource::new(raw).ok()?;
    let decoder = rawler::get_decoder(&source).ok()?;
    let params = RawDecodeParams::default();
    if let Ok(Some(img)) = decoder.thumbnail_image(&source, &params) {
        return Some(img);
    }
    if let Ok(Some(img)) = decoder.preview_image(&source, &params) {
        return Some(img);
    }
    decoder.full_image(&source, &params).ok().flatten()
}

/// Encode an image to a JPEG `data:` URL (base64). Thumbnails only — JPEG is
/// 8-bit, so a 16-bit develop is down-converted first.
fn to_jpeg_data_url(img: &DynamicImage) -> Result<String> {
    let rgb8 = DynamicImage::ImageRgb8(img.to_rgb8());
    let mut bytes: Vec<u8> = Vec::new();
    rgb8.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
        .context("encoding thumbnail JPEG")?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:image/jpeg;base64,{b64}"))
}

/// A downscaled editing preview ready for GPU upload.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPayload {
    pub width: u32,
    pub height: u32,
    /// Base64 of the raw RGB pixels as little-endian **u16** (sRGB-encoded,
    /// 0..=65535, row 0 at top). The frontend uploads this to a float/half-float
    /// WebGL texture (falling back to 8-bit) — richer than an 8-bit PNG so the
    /// stacked tone ops don't band on hard pushes.
    pub data: String,
}

/// Decode + develop a raw file and return a downscaled 16-bit preview.
///
/// The develop is downscaled to [`PREVIEW_MAX_EDGE`] inside the owned pipeline
/// (the area-averaging demosaic produces the reduced image directly), so the
/// buffer never crosses IPC at full sensor resolution.
pub fn preview(path: &str) -> Result<PreviewPayload> {
    let developed = develop::develop_raw(Path::new(path), PREVIEW_MAX_EDGE)?;
    let (width, height) = (developed.width(), developed.height());
    let rgb16 = developed.to_rgb16();
    let samples = rgb16.as_raw(); // &[u16], length = width*height*3

    // Pack as little-endian bytes for a compact, endian-stable transfer.
    let mut bytes = vec![0u8; samples.len() * 2];
    for (i, &v) in samples.iter().enumerate() {
        bytes[i * 2] = (v & 0xff) as u8;
        bytes[i * 2 + 1] = (v >> 8) as u8;
    }
    let data = base64::engine::general_purpose::STANDARD.encode(&bytes);

    Ok(PreviewPayload {
        width,
        height,
        data,
    })
}

/// Produce a small JPEG thumbnail `data:` URL for the grid.
///
/// Cheapest source wins: camera-JPEG sidecar (Fuji RAW+JPEG shooters), then the
/// preview JPEG embedded in the raw itself (most formats carry one), and only
/// as a last resort a downscaled raw develop.
pub fn thumbnail(raw_path: &str) -> Result<String> {
    let raw = Path::new(raw_path);
    let source = match sidecar_jpeg(raw) {
        Some(jpg) => image::open(&jpg)
            .with_context(|| format!("opening sidecar JPEG {}", jpg.display()))?
            .thumbnail(THUMB_MAX_EDGE, THUMB_MAX_EDGE),
        None => match embedded_jpeg(raw) {
            Some(img) => img.thumbnail(THUMB_MAX_EDGE, THUMB_MAX_EDGE),
            // No sidecar, no embedded preview: develop straight to thumb size.
            None => develop::develop_raw(raw, THUMB_MAX_EDGE)?,
        },
    };
    // JPEG keeps the thumbnail payload small; thumbnails are not edited.
    to_jpeg_data_url(&source)
}
