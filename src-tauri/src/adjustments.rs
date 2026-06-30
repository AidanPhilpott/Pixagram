//! # The shared adjustment definition (CANONICAL SOURCE OF TRUTH)
//!
//! This module is the single spec that describes a non-destructive edit. It is
//! constraint #1 from the project brief: BOTH the GPU live preview and the Rust
//! exporter read from this definition and apply *identical* math, so what you
//! see is exactly what you export.
//!
//! - The **data shape** is defined here once (`Adjustments`). The TypeScript
//!   mirror lives in `src/adjustments/definition.ts` and MUST stay field- and
//!   default-compatible with this struct. (Phase 6+ should codegen the TS from
//!   this struct via `ts-rs` to make divergence impossible; for Phase 1 the
//!   single `exposure` field is mirrored by hand and guarded by the doc below.)
//!
//! - The **math** is specified in prose in [`apply_pixel`] and implemented twice:
//!   here in Rust (used by export) and in GLSL (`src/gl/preview.frag.ts`, used by
//!   the GPU preview). Any change to the math must be made in both places.
//!
//! ## Pipeline (the order operations are applied)
//!
//! Tone operations run in **linear light**; the perceptual contrast curve runs
//! on the gamma-encoded value (where contrast feels natural). Pipeline order:
//!
//! 1. Decode the stored pixel from sRGB gamma to linear.
//! 2. **Exposure** — multiply linear RGB by the gain `2^EV` (linear light).
//! 3. **White balance** — a luminance-preserving chroma shift (temp/tint),
//!    linear light. Relative to the developed image, not a raw Kelvin WB.
//! 4. Re-encode linear back to sRGB gamma and clip to [0, 1].
//! 5. **Contrast** — an S-curve around perceptual mid-grey (0.5) on that value.
//!
//! Later phases insert highlights/shadows, tone curve, HSL, etc. into this
//! pipeline, all driven by additional fields on `Adjustments`. Order matters and
//! is the contract — keep it identical here and in the GLSL shader.

use serde::{Deserialize, Serialize};

/// A complete non-destructive adjustment stack for one photo.
///
/// Field names and defaults are part of the contract with the TypeScript mirror
/// (`src/adjustments/definition.ts`). Keep `#[serde(default)]` on every field so
/// older/partial preset JSON keeps deserializing as the stack grows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Adjustments {
    /// Exposure compensation in stops (EV). 0.0 = unchanged. Range checked by the
    /// UI to roughly [-5, +5]; the math itself is unbounded.
    pub exposure: f32,
    /// Contrast amount in [-1.0, +1.0]. 0.0 = unchanged; positive applies an
    /// S-curve around perceptual mid-grey (steepening mid-tones), negative
    /// compresses tones toward mid-grey. The UI shows this as a familiar
    /// [-100, +100].
    pub contrast: f32,
    /// White-balance temperature in [-1.0, +1.0]. 0.0 = neutral; positive warms
    /// (toward yellow/red), negative cools (toward blue). Relative shift on the
    /// developed image, luminance-preserving. UI shows [-100, +100].
    pub temperature: f32,
    /// White-balance tint in [-1.0, +1.0]. 0.0 = neutral; positive shifts toward
    /// magenta, negative toward green. Luminance-preserving. UI shows [-100, +100].
    pub tint: f32,
}

impl Default for Adjustments {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 0.0,
            temperature: 0.0,
            tint: 0.0,
        }
    }
}

/// Full-scale per-channel chroma strength for temperature (R/B) and tint (G).
/// Tunable constants, not architecture — but they MUST match the GLSL shader.
const WB_TEMP_STRENGTH: f32 = 0.3;
const WB_TINT_STRENGTH: f32 = 0.3;

/// Decode one sRGB-encoded channel value (0..=1) to linear light.
#[inline]
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Encode one linear-light channel value to sRGB gamma (0..=1).
#[inline]
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Hermite smoothstep on a clamped [0,1] input — the S used by contrast.
/// Mirrors GLSL `smoothstep(0.0, 1.0, v)` exactly.
#[inline]
fn smoothstep01(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    v * v * (3.0 - 2.0 * v)
}

/// Rec.709 relative luminance of a linear-light RGB triple.
#[inline]
fn luminance(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// Apply white balance to a linear-light RGB triple: a chroma shift that keeps
/// luminance fixed, so temp/tint change only color, never brightness.
///
/// Temperature warms by lifting red and dropping blue (cools the reverse); tint
/// moves green vs magenta via the green channel. The luminance renormalize makes
/// both axes pure-chroma. **Mirror of `applyWhiteBalance` in the GLSL shader.**
#[inline]
fn apply_white_balance(lin: [f32; 3], temperature: f32, tint: f32) -> [f32; 3] {
    if temperature == 0.0 && tint == 0.0 {
        return lin;
    }
    let gr = 1.0 + WB_TEMP_STRENGTH * temperature;
    let gg = 1.0 - WB_TINT_STRENGTH * tint;
    let gb = 1.0 - WB_TEMP_STRENGTH * temperature;
    let shifted = [lin[0] * gr, lin[1] * gg, lin[2] * gb];

    // Preserve luminance so WB is a pure chroma shift.
    let before = luminance(lin);
    let after = luminance(shifted);
    let scale = if after > 1e-6 { before / after } else { 1.0 };
    [shifted[0] * scale, shifted[1] * scale, shifted[2] * scale]
}

/// Apply the contrast S-curve to one perceptual (gamma-encoded) channel value.
///
/// Pivots at perceptual mid-grey (0.5), which is a fixed point at every amount.
/// Positive blends toward an S (steeper mid-tones); negative blends toward a
/// half-slope line (tones compressed toward mid). Monotonic on `[-1, 1]`.
/// **Mirror of `contrastCurve` in the GLSL shader — edit both together.**
#[inline]
fn apply_contrast(v: f32, c: f32) -> f32 {
    if c >= 0.0 {
        // mix(v, smoothstep(v), c)
        v + (smoothstep01(v) - v) * c
    } else {
        // mix(v, 0.5 + (v - 0.5) * 0.5, -c)
        let soft = 0.5 + (v - 0.5) * 0.5;
        v + (soft - v) * (-c)
    }
}

/// Apply the full adjustment stack to a single sRGB-encoded RGB pixel.
///
/// Input and output channels are normalized to `0.0..=1.0`. This is the Rust
/// half of the shared math; the GLSL fragment shader implements the same steps
/// in the same order. **If you edit this, edit the shader too.**
#[inline]
pub fn apply_pixel(adj: &Adjustments, rgb: [f32; 3]) -> [f32; 3] {
    let gain = 2.0_f32.powf(adj.exposure);
    // Exposure in linear light.
    let lin = [
        srgb_to_linear(rgb[0]) * gain,
        srgb_to_linear(rgb[1]) * gain,
        srgb_to_linear(rgb[2]) * gain,
    ];
    // White balance (luminance-preserving chroma shift), still linear.
    let lin = apply_white_balance(lin, adj.temperature, adj.tint);

    let mut out = [0.0_f32; 3];
    for i in 0..3 {
        // Encode to the perceptual domain, then contrast S-curve on it.
        let v = linear_to_srgb(lin[i].max(0.0)).clamp(0.0, 1.0);
        out[i] = apply_contrast(v, adj.contrast).clamp(0.0, 1.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_at_zero_ev() {
        let adj = Adjustments::default();
        let px = [0.2, 0.5, 0.8];
        let out = apply_pixel(&adj, px);
        for i in 0..3 {
            assert!((out[i] - px[i]).abs() < 1e-4, "channel {i} drifted");
        }
    }

    #[test]
    fn plus_one_stop_doubles_linear_midtone() {
        // A mid-grey at +1 EV should roughly double in linear light.
        let adj = Adjustments { exposure: 1.0, ..Default::default() };
        let mid = 0.5_f32;
        let out = apply_pixel(&adj, [mid, mid, mid]);
        let expected = linear_to_srgb((srgb_to_linear(mid) * 2.0).min(1.0));
        assert!((out[0] - expected).abs() < 1e-4);
    }

    #[test]
    fn positive_contrast_pushes_away_from_midgrey() {
        // Above mid brightens, below mid darkens (perceptual S-curve).
        let adj = Adjustments { contrast: 0.5, ..Default::default() };
        let hi = apply_pixel(&adj, [0.8, 0.8, 0.8]);
        let lo = apply_pixel(&adj, [0.1, 0.1, 0.1]);
        assert!(hi[0] > 0.8, "highlight should brighten, got {}", hi[0]);
        assert!(lo[0] < 0.1, "shadow should darken, got {}", lo[0]);
    }

    #[test]
    fn negative_contrast_compresses_toward_mid() {
        // Full negative contrast pulls extremes toward perceptual mid-grey
        // without collapsing them to a single value.
        let adj = Adjustments { contrast: -1.0, ..Default::default() };
        let black = apply_pixel(&adj, [0.0; 3]);
        let white = apply_pixel(&adj, [1.0; 3]);
        assert!(black[0] > 0.0 && black[0] < 0.5, "black lifted, got {}", black[0]);
        assert!(white[0] < 1.0 && white[0] > 0.5, "white pulled, got {}", white[0]);
    }

    #[test]
    fn mid_grey_is_the_contrast_pivot() {
        // Perceptual mid-grey (0.5) is a fixed point at every contrast amount.
        for c in [-1.0, -0.3, 0.3, 1.0] {
            let adj = Adjustments { contrast: c, ..Default::default() };
            let out = apply_pixel(&adj, [0.5; 3]);
            assert!((out[0] - 0.5).abs() < 1e-4, "mid-grey moved at c={c}: {}", out[0]);
        }
    }

    #[test]
    fn white_balance_preserves_linear_luminance() {
        let lin = [0.3, 0.5, 0.7];
        let before = luminance(lin);
        for (t, ti) in [(0.5, 0.0), (0.0, 0.5), (-0.4, 0.3), (0.8, -0.6)] {
            let out = apply_white_balance(lin, t, ti);
            assert!(
                (luminance(out) - before).abs() < 1e-4,
                "luminance drifted at temp={t} tint={ti}"
            );
        }
    }

    #[test]
    fn warm_temperature_favors_red_over_blue() {
        let grey = [0.5; 3];
        let warm = apply_white_balance(grey, 0.6, 0.0);
        assert!(warm[0] > warm[2], "warm should lift red above blue");
        let cool = apply_white_balance(grey, -0.6, 0.0);
        assert!(cool[2] > cool[0], "cool should lift blue above red");
    }

    #[test]
    fn tint_moves_green_vs_magenta() {
        let grey = [0.5; 3];
        let magenta = apply_white_balance(grey, 0.0, 0.6);
        assert!(magenta[1] < magenta[0] && magenta[1] < magenta[2], "magenta dips green");
        let green = apply_white_balance(grey, 0.0, -0.6);
        assert!(green[1] > green[0] && green[1] > green[2], "green tint lifts green");
    }
}
