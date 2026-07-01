//! Owned raw develop pipeline — **camera-general**, not Fuji-specific.
//!
//! ## Why this exists
//! rawler 0.7.2's `RawDevelop` demosaic distorts the X-Trans channel balance: it
//! inflates green ~1.8× relative to red/blue (the 20-of-36 green oversampling is
//! not normalized), so the camera's *correct* as-shot white balance can no longer
//! neutralize the result and everything renders green. Proven by reading the raw
//! sensor mosaic directly: the camera's WB neutralizes the raw sites (R/G, B/G ≈
//! 1.0) but not rawler's demosaiced output (≈ 0.5). See docs/INSIGHTS.md.
//!
//! So we use rawler **only to unpack the raw mosaic and read metadata** (black/
//! white levels, white balance, color matrix, CFA pattern), then run the standard
//! develop ourselves, in the canonical order every correct raw engine uses:
//!
//! ```text
//!   mosaic → black-subtract & normalize → demosaic → white balance
//!          → camera-RGB → sRGB matrix → baseline exposure → sRGB gamma
//! ```
//!
//! Because each step is driven by the file's own metadata and CFA pattern, this is
//! correct-by-construction for any camera (Bayer or X-Trans), which is the whole
//! point: open any raw and see representative color immediately.
//!
//! ## Demosaic (Phase 1 baseline)
//! Previews are downscaled anyway, so we demosaic by **area-averaging**: each
//! output pixel averages the real R/G/B sensor sites within its source footprint
//! (expanded to at least one CFA tile so every color is represented). This is
//! color-correct by construction and naturally downscales. It is softer than a
//! full-resolution demosaic (e.g. Markesteijn) — a high-quality full-res path for
//! export is deferred (see docs/HANDOFF.md).

use std::path::Path;

use anyhow::{anyhow, Result};
use image::{DynamicImage, ImageBuffer, Rgb};
use rawler::rawimage::{RawImage, RawImageData};

/// sRGB (linear, D65) primaries → CIE XYZ. Used to build the camera→sRGB matrix.
const SRGB_TO_XYZ: [[f32; 3]; 3] = [
    [0.4124564, 0.3575761, 0.1804375],
    [0.2126729, 0.7151522, 0.0721750],
    [0.0193339, 0.1191920, 0.9503041],
];

/// Baseline exposure applied during develop, in stops, as a linear gain `2^EV`.
///
/// A pure linear→sRGB develop is markedly darker than the camera's tone-curved
/// JPEG (and Fuji rates ISO conservatively), so without this the default looks
/// under-exposed even though the color is correct. This is a flat profile-style
/// lift — Phase 2's tone curve will supersede it — and the user's exposure slider
/// is applied *on top* of the developed image. Tuned against the camera JPEGs.
const BASELINE_EXPOSURE_EV: f32 = 1.7;

/// Decode a raw file and develop it to a downscaled sRGB image whose longest edge
/// is approximately `max_edge`. This is the single owned color path used by both
/// the editing preview and the (sidecar-less) thumbnail fallback.
pub fn develop_raf(path: &Path, max_edge: u32) -> Result<DynamicImage> {
    let raw = rawler::decode_file(path)
        .map_err(|e| anyhow!("raw decode failed for {}: {e}", path.display()))?;
    develop_rawimage(&raw, max_edge)
        .ok_or_else(|| anyhow!("develop produced no image for {}", path.display()))
}

fn develop_rawimage(raw: &RawImage, max_edge: u32) -> Option<DynamicImage> {
    let data = match &raw.data {
        RawImageData::Integer(d) => d,
        // Float raws (some DNGs) would need a parallel path; not our cameras yet.
        RawImageData::Float(_) => return None,
    };
    let stride = raw.width; // full sensor row stride, including masked borders
    let cfa = &raw.camera.cfa;

    // Visible region: the non-masked active area excludes optical-black borders.
    // Working inside it keeps black bars and dead pixels out of the develop.
    let (x0, y0, rw, rh) = match raw.active_area {
        Some(r) => (r.p.x, r.p.y, r.d.w, r.d.h),
        None => (0, 0, raw.width, raw.height),
    };
    if rw == 0 || rh == 0 {
        return None;
    }

    // White balance, green-normalized (G = 1). wb_coeffs is [R, G, B, G2].
    let wb = [
        raw.wb_coeffs[0] / raw.wb_coeffs[1],
        1.0,
        raw.wb_coeffs[2] / raw.wb_coeffs[1],
    ];
    let wb = wb.map(|c| if c.is_finite() && c > 0.0 { c } else { 1.0 });

    // camera-native RGB → linear sRGB. The camera matrix maps XYZ→camera; compose
    // with sRGB→XYZ, normalize rows (so neutral camera RGB → neutral sRGB), invert.
    let xyz2cam = pick_color_matrix(raw)?;
    let cam2srgb = mat_inv(normalize_rows(mat_mul(xyz2cam, SRGB_TO_XYZ)));

    // Per-position black level (a small repeating pattern) and the white level.
    let bl = &raw.blacklevel;
    let (bw, bh) = (bl.width.max(1), bl.height.max(1));
    let black_levels: Vec<f32> = bl.levels.iter().map(|r| r.as_f32()).collect();
    let black_at = |ar: usize, ac: usize| -> f32 {
        if black_levels.is_empty() {
            0.0
        } else {
            black_levels[((ar % bh) * bw + (ac % bw)) % black_levels.len()]
        }
    };
    let white = raw
        .whitelevel
        .as_vec()
        .first()
        .copied()
        .unwrap_or(65535.0);

    let gain = 2.0_f32.powf(BASELINE_EXPOSURE_EV);

    // Output size: scale the longest visible edge down to ~max_edge.
    let scale = (rw.max(rh) as f32 / max_edge as f32).max(1.0);
    let out_w = ((rw as f32 / scale).round() as u32).max(1);
    let out_h = ((rh as f32 / scale).round() as u32).max(1);

    // Expand each output footprint to at least one CFA tile so every color is
    // represented (X-Trans needs 6×6, Bayer 2×2).
    let period_x = cfa.width.max(1);
    let period_y = cfa.height.max(1);

    // 16-bit output: the develop computes in f32, so emitting u16 (not u8) hands
    // the GPU preview far more tonal headroom — stacked tone ops won't band.
    let mut out: ImageBuffer<Rgb<u16>, Vec<u16>> = ImageBuffer::new(out_w, out_h);
    for oy in 0..out_h {
        let mut sy0 = (oy as f32 * scale) as usize;
        let mut sy1 = (((oy + 1) as f32 * scale) as usize).max(sy0 + 1);
        if sy1 - sy0 < period_y {
            sy1 = (sy0 + period_y).min(rh);
            sy0 = sy1.saturating_sub(period_y);
        }
        for ox in 0..out_w {
            let mut sx0 = (ox as f32 * scale) as usize;
            let mut sx1 = (((ox + 1) as f32 * scale) as usize).max(sx0 + 1);
            if sx1 - sx0 < period_x {
                sx1 = (sx0 + period_x).min(rw);
                sx0 = sx1.saturating_sub(period_x);
            }

            // Average each color's sensor sites within the footprint.
            let mut sum = [0.0f32; 3];
            let mut cnt = [0u32; 3];
            for sy in sy0..sy1 {
                let ar = y0 + sy;
                let row = ar * stride;
                for sx in sx0..sx1 {
                    let ac = x0 + sx;
                    let c = cfa.color_at(ar, ac);
                    if c < 3 {
                        let v = data[row + ac] as f32 - black_at(ar, ac);
                        sum[c] += v.max(0.0);
                        cnt[c] += 1;
                    }
                }
            }

            // Normalize to linear [0,1], then white-balance.
            let mut cam = [0.0f32; 3];
            for c in 0..3 {
                let denom = (white - black_at(y0 + sy0, x0 + sx0)).max(1.0);
                let mean = if cnt[c] > 0 { sum[c] / cnt[c] as f32 } else { 0.0 };
                cam[c] = (mean / denom) * wb[c];
            }

            // camera RGB → linear sRGB, baseline exposure, sRGB gamma.
            let mut px = [0u16; 3];
            for i in 0..3 {
                let lin = (cam2srgb[i][0] * cam[0]
                    + cam2srgb[i][1] * cam[1]
                    + cam2srgb[i][2] * cam[2])
                    * gain;
                px[i] = (linear_to_srgb(lin.clamp(0.0, 1.0)) * 65535.0).round() as u16;
            }
            out.put_pixel(ox, oy, Rgb(px));
        }
    }

    Some(DynamicImage::ImageRgb16(out))
}

/// Choose the camera color matrix (XYZ→camera). Prefer the D65 illuminant — the
/// daylight reference most adjustments assume — falling back to whatever exists.
/// (Phase 2: interpolate between illuminants by estimated color temperature.)
fn pick_color_matrix(raw: &RawImage) -> Option<[[f32; 3]; 3]> {
    let prefer_d65 = raw
        .color_matrix
        .iter()
        .find(|(illum, _)| format!("{illum:?}").contains("D65"));
    let (_, m) = prefer_d65.or_else(|| raw.color_matrix.iter().next())?;
    if m.len() < 9 {
        return None;
    }
    Some([
        [m[0], m[1], m[2]],
        [m[3], m[4], m[5]],
        [m[6], m[7], m[8]],
    ])
}

#[inline]
fn linear_to_srgb(x: f32) -> f32 {
    if x <= 0.003_130_8 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

fn mat_mul(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut o = [[0.0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

fn normalize_rows(mut m: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    for r in m.iter_mut() {
        let s = r[0] + r[1] + r[2];
        if s.abs() > 1e-9 {
            for v in r.iter_mut() {
                *v /= s;
            }
        }
    }
    m
}

fn mat_inv(m: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let inv_det = if det.abs() > 1e-12 { 1.0 / det } else { 0.0 };
    [
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv_det,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv_det,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv_det,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv_det,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv_det,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv_det,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv_det,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv_det,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv_det,
        ],
    ]
}
