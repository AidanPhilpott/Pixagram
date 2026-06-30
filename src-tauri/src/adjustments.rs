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
//! 5. **Tone regions** — highlights / shadows / whites / blacks, each a
//!    smoothstep-masked lift on the perceptual value (per channel).
//! 6. **Contrast** — an S-curve around perceptual mid-grey (0.5) on that value.
//! 7. **Saturation / vibrance** — scale each pixel's chroma (distance from its
//!    grey/luma), across all three channels at once.
//! 8. **HSL** — per-hue-band hue/saturation/luminance: convert to HSL, blend the
//!    adjacent bands' adjustments at the pixel's hue, apply, convert back.
//!
//! Later phases insert the tone curve into this pipeline, driven by additional
//! fields on `Adjustments`. Order matters and is the contract — keep it identical
//! here and in the GLSL shader.

use serde::{Deserialize, Serialize};

/// One color band of the HSL panel — hue rotation, saturation, and luminance,
/// each in `[-1.0, +1.0]` (0 = unchanged). There are 8 bands (see `HSL_BANDS` in
/// the TS mirror); a pixel blends the two whose hues bracket its own.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HslBand {
    pub hue: f32,
    pub sat: f32,
    pub lum: f32,
}

impl Default for HslBand {
    fn default() -> Self {
        Self { hue: 0.0, sat: 0.0, lum: 0.0 }
    }
}

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
    /// Highlights in [-1.0, +1.0]. 0.0 = unchanged; positive brightens the bright
    /// mid-tones, negative recovers them. UI shows [-100, +100].
    pub highlights: f32,
    /// Shadows in [-1.0, +1.0]. 0.0 = unchanged; positive lifts the dark
    /// mid-tones, negative deepens them. UI shows [-100, +100].
    pub shadows: f32,
    /// Whites in [-1.0, +1.0]. 0.0 = unchanged; positive pushes the near-white
    /// end up (toward clipping), negative pulls it back. UI shows [-100, +100].
    pub whites: f32,
    /// Blacks in [-1.0, +1.0]. 0.0 = unchanged; positive lifts the near-black
    /// end, negative crushes it. UI shows [-100, +100].
    pub blacks: f32,
    /// Saturation in [-1.0, +1.0]. 0.0 = unchanged; -1.0 = greyscale, +1.0 doubles
    /// chroma. Scales every pixel's distance from its grey uniformly. UI [-100,+100].
    pub saturation: f32,
    /// Vibrance in [-1.0, +1.0]. 0.0 = unchanged. Like saturation but weighted by
    /// `(1 - pixel saturation)`, so muted colors move more than already-vivid ones.
    /// UI shows [-100, +100].
    pub vibrance: f32,
    /// Per-hue HSL adjustments, one entry per color band (red, orange, yellow,
    /// green, aqua, blue, purple, magenta — centers in `hsl_band_adjust`).
    pub hsl: [HslBand; 8],
}

impl Default for Adjustments {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 0.0,
            temperature: 0.0,
            tint: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            saturation: 0.0,
            vibrance: 0.0,
            hsl: [HslBand { hue: 0.0, sat: 0.0, lum: 0.0 }; 8],
        }
    }
}

/// Hue rotation (degrees) a full HSL hue slider applies. Tunable; match the shader.
const HSL_HUE_DEG: f32 = 30.0;
/// Lightness shift a full HSL luminance slider applies. Tunable; match the shader.
const HSL_LUM_STRENGTH: f32 = 0.5;

/// Full-scale per-channel chroma strength for temperature (R/B) and tint (G).
/// Tunable constants, not architecture — but they MUST match the GLSL shader.
const WB_TEMP_STRENGTH: f32 = 0.3;
const WB_TINT_STRENGTH: f32 = 0.3;

/// Full-scale lift the tone-region controls can apply to the perceptual value.
/// Tunable; MUST match `TONE_STRENGTH` in the GLSL shader.
const TONE_STRENGTH: f32 = 0.2;

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

/// General Hermite smoothstep between two edges — mirrors GLSL `smoothstep`.
#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Apply the four tone-region controls to one perceptual channel value.
///
/// Each control owns a smoothstep-weighted band of the tonal range and adds a
/// lift scaled by its amount: highlights/whites toward white, shadows/blacks
/// toward black, with whites/blacks biased to the extremes. Per channel, like
/// contrast. **Mirror of `toneRegions` in the GLSL shader — edit both together.**
#[inline]
fn apply_tone_regions(v: f32, adj: &Adjustments) -> f32 {
    let w_highlights = smoothstep(0.4, 0.9, v);
    let w_shadows = 1.0 - smoothstep(0.1, 0.6, v);
    let w_whites = smoothstep(0.6, 1.0, v);
    let w_blacks = 1.0 - smoothstep(0.0, 0.4, v);
    let delta = adj.highlights * w_highlights
        + adj.shadows * w_shadows
        + adj.whites * w_whites
        + adj.blacks * w_blacks;
    (v + TONE_STRENGTH * delta).clamp(0.0, 1.0)
}

/// Scale a perceptual RGB pixel's chroma (its distance from its own grey/luma).
///
/// Saturation scales every pixel equally; vibrance weights the scale by
/// `(1 - pixel_saturation)` so muted colors move more than already-vivid ones.
/// The two combine multiplicatively (0/0 = identity). Cross-channel, so this runs
/// on the whole triple. **Mirror of `applySaturation` in the GLSL shader.**
#[inline]
fn apply_saturation(rgb: [f32; 3], saturation: f32, vibrance: f32) -> [f32; 3] {
    if saturation == 0.0 && vibrance == 0.0 {
        return rgb;
    }
    let luma = luminance(rgb);
    let max_c = rgb[0].max(rgb[1]).max(rgb[2]);
    let min_c = rgb[0].min(rgb[1]).min(rgb[2]);
    let pixel_sat = max_c - min_c;
    let factor = (1.0 + saturation) * (1.0 + vibrance * (1.0 - pixel_sat));
    [
        (luma + (rgb[0] - luma) * factor).clamp(0.0, 1.0),
        (luma + (rgb[1] - luma) * factor).clamp(0.0, 1.0),
        (luma + (rgb[2] - luma) * factor).clamp(0.0, 1.0),
    ]
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Convert a perceptual RGB pixel to HSL. Returns hue in degrees `[0, 360)`,
/// saturation and lightness in `[0, 1]`. Mirror of GLSL `rgb2hsl`.
#[inline]
fn rgb_to_hsl(rgb: [f32; 3]) -> (f32, f32, f32) {
    let (r, g, b) = (rgb[0], rgb[1], rgb[2]);
    let maxc = r.max(g).max(b);
    let minc = r.min(g).min(b);
    let l = (maxc + minc) * 0.5;
    let d = maxc - minc;
    if d <= 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let mut h = if maxc == r {
        (g - b) / d
    } else if maxc == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h = (h * 60.0).rem_euclid(360.0);
    (h, s, l)
}

/// Convert HSL (hue degrees, sat, lightness) back to RGB. Mirror of GLSL `hsl2rgb`.
#[inline]
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = if hp < 1.0 {
        (c, x, 0.0)
    } else if hp < 2.0 {
        (x, c, 0.0)
    } else if hp < 3.0 {
        (0.0, c, x)
    } else if hp < 4.0 {
        (0.0, x, c)
    } else if hp < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = l - c * 0.5;
    [r1 + m, g1 + m, b1 + m]
}

/// Blend the two color bands whose centers bracket `h` (degrees), returning the
/// interpolated (hue, sat, lum) adjustment. Band centers: red 0, orange 30,
/// yellow 60, green 120, aqua 180, blue 240, purple 270, magenta 300 (wrapping
/// 300→360 back to red). **Segment thresholds MUST match the GLSL shader.**
#[inline]
fn hsl_band_adjust(h: f32, hsl: &[HslBand; 8]) -> (f32, f32, f32) {
    let (i0, i1, t) = if h < 30.0 {
        (0, 1, h / 30.0)
    } else if h < 60.0 {
        (1, 2, (h - 30.0) / 30.0)
    } else if h < 120.0 {
        (2, 3, (h - 60.0) / 60.0)
    } else if h < 180.0 {
        (3, 4, (h - 120.0) / 60.0)
    } else if h < 240.0 {
        (4, 5, (h - 180.0) / 60.0)
    } else if h < 270.0 {
        (5, 6, (h - 240.0) / 30.0)
    } else if h < 300.0 {
        (6, 7, (h - 270.0) / 30.0)
    } else {
        (7, 0, (h - 300.0) / 60.0)
    };
    let (a, b) = (hsl[i0], hsl[i1]);
    (lerp(a.hue, b.hue, t), lerp(a.sat, b.sat, t), lerp(a.lum, b.lum, t))
}

/// Apply the per-hue HSL adjustments to a perceptual RGB pixel. No-op when every
/// band is neutral. Adjustments fade out on near-neutral pixels (no real hue).
/// **Mirror of `applyHsl` in the GLSL shader.**
#[inline]
fn apply_hsl(rgb: [f32; 3], hsl: &[HslBand; 8]) -> [f32; 3] {
    if hsl
        .iter()
        .all(|b| b.hue == 0.0 && b.sat == 0.0 && b.lum == 0.0)
    {
        return rgb;
    }
    let (h, s, l) = rgb_to_hsl(rgb);
    let color_mask = smoothstep(0.0, 0.15, s);
    let (dh, ds, dl) = hsl_band_adjust(h, hsl);
    let h2 = (h + dh * HSL_HUE_DEG * color_mask).rem_euclid(360.0);
    let s2 = (s * (1.0 + ds * color_mask)).clamp(0.0, 1.0);
    let l2 = (l + dl * HSL_LUM_STRENGTH * color_mask).clamp(0.0, 1.0);
    hsl_to_rgb(h2, s2, l2)
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
        // Encode to the perceptual domain, then shape tone regions + contrast.
        let v = linear_to_srgb(lin[i].max(0.0)).clamp(0.0, 1.0);
        let v = apply_tone_regions(v, adj);
        out[i] = apply_contrast(v, adj.contrast).clamp(0.0, 1.0);
    }
    // Saturation / vibrance — cross-channel, so it runs on the whole triple.
    let out = apply_saturation(out, adj.saturation, adj.vibrance);
    // HSL — per-hue hue/sat/luminance.
    apply_hsl(out, &adj.hsl)
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

    #[test]
    fn highlights_act_on_highlights_not_shadows() {
        let adj = Adjustments { highlights: 1.0, ..Default::default() };
        let hi = apply_pixel(&adj, [0.8; 3]);
        let lo = apply_pixel(&adj, [0.1; 3]);
        assert!(hi[0] > 0.8, "highlights+ should brighten a highlight");
        assert!((lo[0] - 0.1).abs() < 0.02, "a deep shadow should be barely touched");
    }

    #[test]
    fn shadows_act_on_shadows_not_highlights() {
        let adj = Adjustments { shadows: 1.0, ..Default::default() };
        let lo = apply_pixel(&adj, [0.15; 3]);
        let hi = apply_pixel(&adj, [0.85; 3]);
        assert!(lo[0] > 0.15, "shadows+ should lift a shadow");
        assert!((hi[0] - 0.85).abs() < 0.03, "a highlight should be barely touched");
    }

    #[test]
    fn whites_and_blacks_move_the_endpoints() {
        let crush = Adjustments { blacks: -1.0, ..Default::default() };
        let near_black = apply_pixel(&crush, [0.1; 3]);
        assert!(near_black[0] < 0.1, "blacks- should crush near-black");

        let lift = Adjustments { whites: 1.0, ..Default::default() };
        let near_white = apply_pixel(&lift, [0.9; 3]);
        assert!(near_white[0] > 0.9, "whites+ should brighten near-white");
    }

    #[test]
    fn saturation_scales_chroma() {
        let color = [0.7, 0.4, 0.4]; // reddish
        let grey = apply_saturation(color, -1.0, 0.0);
        assert!(
            (grey[0] - grey[1]).abs() < 1e-4 && (grey[1] - grey[2]).abs() < 1e-4,
            "full -saturation collapses to grey"
        );
        let boosted = apply_saturation(color, 0.5, 0.0);
        assert!(
            boosted[0] - boosted[1] > color[0] - color[1],
            "saturation+ widens chroma"
        );
    }

    #[test]
    fn vibrance_favors_unsaturated_pixels() {
        // Same vibrance scales a muted pixel's chroma more than a vivid one's.
        let muted = [0.55, 0.45, 0.45];
        let vivid = [0.70, 0.20, 0.20];
        let m = apply_saturation(muted, 0.0, 1.0);
        let v = apply_saturation(vivid, 0.0, 1.0);
        let ratio_m = (m[0] - m[1]) / (muted[0] - muted[1]);
        let ratio_v = (v[0] - v[1]) / (vivid[0] - vivid[1]);
        assert!(ratio_m > ratio_v, "vibrance lifts muted ({ratio_m}) more than vivid ({ratio_v})");
    }

    #[test]
    fn rgb_hsl_round_trips() {
        for px in [[0.8, 0.2, 0.2], [0.2, 0.6, 0.3], [0.3, 0.3, 0.9], [0.5; 3]] {
            let (h, s, l) = rgb_to_hsl(px);
            let back = hsl_to_rgb(h, s, l);
            for i in 0..3 {
                assert!((back[i] - px[i]).abs() < 1e-4, "round-trip drifted on {px:?}");
            }
        }
    }

    #[test]
    fn hsl_targets_only_the_matching_band() {
        // Fully desaturate the red band; a red pixel goes grey, a blue one is left.
        let mut hsl = [HslBand::default(); 8];
        hsl[0].sat = -1.0; // red band
        let adj = Adjustments { hsl, ..Default::default() };

        let red = apply_pixel(&adj, [0.8, 0.2, 0.2]); // hue ~0
        let red_spread = red[0].max(red[1]).max(red[2]) - red[0].min(red[1]).min(red[2]);
        assert!(red_spread < 0.05, "red band should desaturate reds, got spread {red_spread}");

        let blue = apply_pixel(&adj, [0.2, 0.2, 0.8]); // hue ~240
        let blue_spread = blue[0].max(blue[1]).max(blue[2]) - blue[0].min(blue[1]).min(blue[2]);
        assert!(blue_spread > 0.5, "blue should be untouched, got spread {blue_spread}");
    }
}
