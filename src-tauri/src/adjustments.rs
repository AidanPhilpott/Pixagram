//! # The shared adjustment definition (CANONICAL SOURCE OF TRUTH)
//!
//! This module is the single spec that describes a non-destructive edit. It is
//! constraint #1 from the project brief: BOTH the GPU live preview and the Rust
//! exporter read from this definition and apply *identical* math, so what you
//! see is exactly what you export.
//!
//! - The **data shape** is defined here once (`Adjustments`, `HslBand`,
//!   `CurvePoint`). The TypeScript types are **generated from these structs by
//!   ts-rs** (`#[derive(TS)]` → `src/adjustments/generated/`), so they can't
//!   drift from Rust. Regenerate after any struct change with `npm run gen:types`
//!   (or `cargo test … export_bindings`); `src/adjustments/definition.ts` adds
//!   only the runtime helpers (defaults, ranges) on top of the generated types.
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
//! 7. **Tone curve** — a user-drawn monotonic spline, evaluated per channel.
//! 8. **Saturation / vibrance** — scale each pixel's chroma (distance from its
//!    grey/luma), across all three channels at once.
//! 9. **HSL** — per-hue-band hue/saturation/luminance: convert to HSL, blend the
//!    adjacent bands' adjustments at the pixel's hue, apply, convert back.
//! 10. **Color chrome** — deepen highly saturated colors (darken + densify in
//!     proportion to the pixel's own saturation), with an extra blue-weighted
//!     term. Emulates Fujifilm's Color Chrome Effect / Color Chrome FX Blue.
//!
//! This is the full color pipeline. Order matters and is the contract —
//! keep it identical here and in the GLSL shader.
//!
//! **Grain is spatial and runs OUTSIDE `apply_pixel`** (it needs pixel
//! coordinates, which per-pixel color math doesn't have). Like rotation it is a
//! documented lockstep pair: [`apply_grain`] here (the exporter must call it per
//! pixel *after* `apply_pixel`) and the grain block at the end of the GLSL
//! shader. Both sides quantize image-space coordinates to grain cells and hash
//! them with the same function, so the character matches; because grain is
//! stochastic and the preview is downscaled, preview grain matches the export in
//! look, not pixel-for-pixel (the one deliberate WYSIWYG relaxation).
//!
//! **Geometry (rotation) is NOT part of `apply_pixel`.** The `rotation` field is
//! quarter-turn geometry applied once per image, *after* all pixel math: the
//! exporter calls [`crate::export::apply_rotation`] (`image` crate
//! rotate90/180/270) and the GPU preview rotates the quad's texture coordinates
//! and swaps the canvas aspect (`applyGeometry` in `src/gl/PreviewRenderer.ts`).
//! Both are clockwise quarter-turns — keep the two sides in lockstep.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One color band of the HSL panel — hue rotation, saturation, and luminance,
/// each in `[-1.0, +1.0]` (0 = unchanged). There are 8 bands (see `HSL_BANDS` in
/// the TS mirror); a pixel blends the two whose hues bracket its own.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, export_to = "../../src/adjustments/generated/")]
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

/// One tone-curve control point; both axes in `[0, 1]` (input x → output y, on
/// the perceptual value). The TS mirror is generated from this struct by ts-rs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/adjustments/generated/")]
pub struct CurvePoint {
    pub x: f32,
    pub y: f32,
}

/// Most control points a curve can have (UI cap); keeps `curve_eval` allocation-free.
const MAX_CURVE_POINTS: usize = 16;

/// A complete non-destructive adjustment stack for one photo.
///
/// Field names and defaults are part of the contract with the TypeScript mirror
/// (`src/adjustments/definition.ts`). Keep `#[serde(default)]` on every field so
/// older/partial preset JSON keeps deserializing as the stack grows.
///
/// Not `Copy`: the tone curve is a `Vec`. Pass by `&` (as `apply_pixel` does).
/// The TS mirror is generated from this struct by ts-rs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, export_to = "../../src/adjustments/generated/")]
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
    /// Always length 8 (the UI guarantees it); a `Vec` so ts-rs emits `Array`.
    pub hsl: Vec<HslBand>,
    /// Tone-curve control points, sorted by `x`, both axes in [0,1]. Default is
    /// the identity line `[(0,0), (1,1)]`. Applied per channel after contrast.
    pub curve: Vec<CurvePoint>,
    /// Color Chrome Effect strength in [0.0, 1.0]. 0.0 = off. Deepens highly
    /// saturated colors (darkens + adds density in proportion to the pixel's own
    /// saturation) for extra tonality where colors would otherwise block up.
    /// Fuji's Weak ≈ 0.5, Strong ≈ 1.0. UI shows [0, 100].
    pub color_chrome: f32,
    /// Color Chrome FX Blue strength in [0.0, 1.0]. 0.0 = off. Same deepening as
    /// `color_chrome` but weighted to blue hues only (skies, blue subjects).
    /// Fuji's Weak ≈ 0.5, Strong ≈ 1.0. UI shows [0, 100].
    pub color_chrome_blue: f32,
    /// Film-grain amount in [0.0, 1.0]. 0.0 = off. Monochromatic luminance grain,
    /// strongest in the midtones, faded at pure black/white like real film.
    /// Fuji's Weak ≈ 0.25, Strong ≈ 0.5. Spatial — applied by [`apply_grain`]
    /// after the color math, NOT inside `apply_pixel`. UI shows [0, 100].
    pub grain: f32,
    /// Grain particle size in [0.0, 1.0]: 0.0 = fine (1px cells), 1.0 = coarse
    /// (3px cells). Fuji's Small ≈ 0.0, Large ≈ 0.75. Only visible when
    /// `grain > 0`. UI shows [0, 100].
    pub grain_size: f32,
    /// Rotation in **clockwise quarter-turns**, `0..=3` (0 = as shot). Geometry,
    /// not color — deliberately absent from `apply_pixel`. Applied once per image
    /// after the pixel math: `export::apply_rotation` on the Rust side, the
    /// quad-UV rotation + canvas aspect swap in `src/gl/PreviewRenderer.ts` on
    /// the preview side. Edit those two together.
    pub rotation: u8,
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
            hsl: vec![HslBand { hue: 0.0, sat: 0.0, lum: 0.0 }; 8],
            curve: vec![
                CurvePoint { x: 0.0, y: 0.0 },
                CurvePoint { x: 1.0, y: 1.0 },
            ],
            color_chrome: 0.0,
            color_chrome_blue: 0.0,
            grain: 0.0,
            grain_size: 0.0,
            rotation: 0,
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

/// Color-chrome full-scale effect: how much a fully saturated pixel is darkened
/// (`CC_LUM`) and densified (`CC_SAT`) at strength 1.0. Tunable; MUST match the
/// GLSL shader.
const CC_LUM_STRENGTH: f32 = 0.12;
const CC_SAT_STRENGTH: f32 = 0.10;

/// Full-scale luminance amplitude of the film grain (strength 1.0, midtone
/// pixel). Tunable; MUST match `GRAIN_STRENGTH` in the GLSL shader.
const GRAIN_STRENGTH: f32 = 0.15;

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
fn hsl_band_adjust(h: f32, hsl: &[HslBand]) -> (f32, f32, f32) {
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
fn apply_hsl(rgb: [f32; 3], hsl: &[HslBand]) -> [f32; 3] {
    // hsl_band_adjust indexes bands 0..=7, so anything shorter is treated as a no-op.
    if hsl.len() < 8 || hsl.iter().all(|b| b.hue == 0.0 && b.sat == 0.0 && b.lum == 0.0) {
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

/// Apply the color-chrome effect to a perceptual RGB pixel: darken and densify
/// in proportion to the pixel's own saturation, so vivid colors gain depth and
/// near-neutrals are untouched. `blue` adds the same deepening weighted to blue
/// hues (a raised-cosine window over 180°–300°, peaking at 240°). Emulates
/// Fujifilm's Color Chrome Effect / Color Chrome FX Blue.
/// **Mirror of `applyColorChrome` in the GLSL shader — edit both together.**
#[inline]
fn apply_color_chrome(rgb: [f32; 3], strength: f32, blue: f32) -> [f32; 3] {
    if strength == 0.0 && blue == 0.0 {
        return rgb;
    }
    let (h, s, l) = rgb_to_hsl(rgb);
    let blue_w = smoothstep(180.0, 240.0, h) * (1.0 - smoothstep(240.0, 300.0, h));
    let amount = (strength + blue * blue_w) * s;
    let l2 = (l * (1.0 - CC_LUM_STRENGTH * amount)).clamp(0.0, 1.0);
    let s2 = (s * (1.0 + CC_SAT_STRENGTH * amount)).clamp(0.0, 1.0);
    hsl_to_rgb(h, s2, l2)
}

/// One value-noise sample for the grain: hash a grain-cell coordinate to
/// [0, 1). Dave Hoskins' `hash12` — chosen because it uses only fract/dot/mul
/// (no `sin`), so the f32 math is reproducible between Rust and the GPU.
/// **Mirror of `hashCell` in the GLSL shader — identical constants.**
#[inline]
fn hash_cell(x: f32, y: f32) -> f32 {
    // GLSL fract() is x - floor(x) (always in [0,1)), not Rust's sign-keeping
    // f32::fract — inputs here are non-negative but keep the exact form.
    let fract = |v: f32| v - v.floor();
    let mut p3 = [fract(x * 0.1031), fract(y * 0.1031), fract(x * 0.1031)];
    let d = p3[0] * (p3[1] + 33.33) + p3[1] * (p3[2] + 33.33) + p3[2] * (p3[0] + 33.33);
    p3[0] += d;
    p3[1] += d;
    p3[2] += d;
    fract((p3[0] + p3[1]) * p3[2])
}

/// Apply film grain to one finished pixel at image coordinates `(x, y)`.
///
/// Spatial — the coordinate-needing counterpart to [`apply_pixel`]; the exporter
/// must call this per pixel AFTER `apply_pixel` (grain sits on top of the final
/// color, like real film). Monochromatic luminance noise, one random value per
/// grain cell (`1 + 2·grain_size` px square), scaled by a midtone mask so pure
/// black/white stay clean. **Mirror of the grain block at the end of the GLSL
/// shader's `main()` — edit both together.** Preview grain is computed at
/// preview resolution, so it matches the export in character, not per-pixel.
#[inline]
pub fn apply_grain(rgb: [f32; 3], x: u32, y: u32, adj: &Adjustments) -> [f32; 3] {
    if adj.grain <= 0.0 {
        return rgb;
    }
    let cell = 1.0 + 2.0 * adj.grain_size.clamp(0.0, 1.0);
    let cx = (x as f32 / cell).floor();
    let cy = (y as f32 / cell).floor();
    let n = hash_cell(cx, cy) * 2.0 - 1.0;
    let luma = luminance(rgb);
    let mask = smoothstep(0.0, 0.15, luma) * (1.0 - smoothstep(0.85, 1.0, luma));
    let d = GRAIN_STRENGTH * adj.grain * n * mask;
    [
        (rgb[0] + d).clamp(0.0, 1.0),
        (rgb[1] + d).clamp(0.0, 1.0),
        (rgb[2] + d).clamp(0.0, 1.0),
    ]
}

/// Evaluate the tone curve at `x` (perceptual, [0,1]) with a monotonic cubic
/// (Fritsch–Carlson) spline through `points`. `points` must be sorted by `x`
/// with ≥2 entries; the identity line `[(0,0),(1,1)]` returns `x` exactly.
/// **Mirror of `evalCurve` in `src/adjustments/curve.ts` — keep them identical.**
fn curve_eval(points: &[CurvePoint], x: f32) -> f32 {
    let n = points.len().min(MAX_CURVE_POINTS);
    if n < 2 {
        return if n == 1 { points[0].y } else { x.clamp(0.0, 1.0) };
    }
    if x <= points[0].x {
        return points[0].y;
    }
    if x >= points[n - 1].x {
        return points[n - 1].y;
    }

    // Secant slopes between consecutive points.
    let mut d = [0.0_f32; MAX_CURVE_POINTS];
    for i in 0..n - 1 {
        let dx = points[i + 1].x - points[i].x;
        d[i] = if dx > 1e-9 { (points[i + 1].y - points[i].y) / dx } else { 0.0 };
    }

    // Initial tangents (averaged), then the Fritsch–Carlson monotonicity clamp.
    let mut m = [0.0_f32; MAX_CURVE_POINTS];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        m[i] = (d[i - 1] + d[i]) * 0.5;
    }
    for i in 0..n - 1 {
        if d[i].abs() < 1e-9 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
        } else {
            let a = m[i] / d[i];
            let b = m[i + 1] / d[i];
            let sq = a * a + b * b;
            if sq > 9.0 {
                let t = 3.0 / sq.sqrt();
                m[i] = t * a * d[i];
                m[i + 1] = t * b * d[i];
            }
        }
    }

    // Hermite-interpolate the segment containing x.
    let mut k = 0;
    while k < n - 2 && x >= points[k + 1].x {
        k += 1;
    }
    let h = points[k + 1].x - points[k].x;
    let t = (x - points[k].x) / h;
    let (t2, t3) = (t * t, t * t * t);
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    let y = h00 * points[k].y + h10 * h * m[k] + h01 * points[k + 1].y + h11 * h * m[k + 1];
    y.clamp(0.0, 1.0)
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
        let v = apply_contrast(v, adj.contrast).clamp(0.0, 1.0);
        // Tone curve (identity line is a no-op).
        out[i] = curve_eval(&adj.curve, v);
    }
    // Saturation / vibrance — cross-channel, so it runs on the whole triple.
    let out = apply_saturation(out, adj.saturation, adj.vibrance);
    // HSL — per-hue hue/sat/luminance.
    let out = apply_hsl(out, &adj.hsl);
    // Color chrome — deepen saturated colors (+ the blue-weighted variant).
    apply_color_chrome(out, adj.color_chrome, adj.color_chrome_blue)
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
        let mut hsl = vec![HslBand::default(); 8];
        hsl[0].sat = -1.0; // red band
        let adj = Adjustments { hsl, ..Default::default() };

        let red = apply_pixel(&adj, [0.8, 0.2, 0.2]); // hue ~0
        let red_spread = red[0].max(red[1]).max(red[2]) - red[0].min(red[1]).min(red[2]);
        assert!(red_spread < 0.05, "red band should desaturate reds, got spread {red_spread}");

        let blue = apply_pixel(&adj, [0.2, 0.2, 0.8]); // hue ~240
        let blue_spread = blue[0].max(blue[1]).max(blue[2]) - blue[0].min(blue[1]).min(blue[2]);
        assert!(blue_spread > 0.5, "blue should be untouched, got spread {blue_spread}");
    }

    #[test]
    fn color_chrome_deepens_saturated_colors_only() {
        let adj = Adjustments { color_chrome: 1.0, ..Default::default() };
        // A vivid red loses luminance (gains depth)…
        let vivid_in = [0.9, 0.15, 0.15];
        let vivid = apply_pixel(&adj, vivid_in);
        assert!(
            luminance(vivid) < luminance(vivid_in) - 0.005,
            "vivid red should darken, {} -> {}",
            luminance(vivid_in),
            luminance(vivid)
        );
        // …while a grey is untouched (amount scales with the pixel's saturation).
        let grey = apply_pixel(&adj, [0.5; 3]);
        assert!((grey[0] - 0.5).abs() < 1e-4, "grey must be untouched, got {}", grey[0]);
    }

    #[test]
    fn color_chrome_blue_targets_blue_hues() {
        let adj = Adjustments { color_chrome_blue: 1.0, ..Default::default() };
        let blue_in = [0.2, 0.3, 0.9];
        let red_in = [0.9, 0.2, 0.2];
        let d_blue = luminance(blue_in) - luminance(apply_pixel(&adj, blue_in));
        let d_red = luminance(red_in) - luminance(apply_pixel(&adj, red_in));
        assert!(d_blue > 0.005, "blue pixel should deepen, delta {d_blue}");
        assert!(d_red.abs() < 1e-4, "red pixel should be untouched, delta {d_red}");
    }

    #[test]
    fn grain_zero_is_identity_and_grain_is_monochrome() {
        let adj = Adjustments::default();
        let px = [0.3, 0.5, 0.7];
        assert_eq!(apply_grain(px, 10, 20, &adj), px, "no grain = exact identity");

        let grainy = Adjustments { grain: 1.0, ..Default::default() };
        let out = apply_grain(px, 10, 20, &grainy);
        // Monochromatic: every channel moves by the same luminance delta.
        let d = [out[0] - px[0], out[1] - px[1], out[2] - px[2]];
        assert!((d[0] - d[1]).abs() < 1e-6 && (d[1] - d[2]).abs() < 1e-6, "grain is luma-only");
        assert!(d[0].abs() <= GRAIN_STRENGTH + 1e-6, "amplitude bounded by GRAIN_STRENGTH");
    }

    #[test]
    fn grain_varies_by_position_fades_at_extremes_and_is_deterministic() {
        let adj = Adjustments { grain: 1.0, ..Default::default() };
        let px = [0.5; 3];
        // Deterministic (same coord, same value) but varies across coords.
        assert_eq!(apply_grain(px, 3, 7, &adj), apply_grain(px, 3, 7, &adj));
        let samples: Vec<f32> = (0..32).map(|i| apply_grain(px, i, i * 13 + 1, &adj)[0]).collect();
        let spread = samples.iter().cloned().fold(f32::MIN, f32::max)
            - samples.iter().cloned().fold(f32::MAX, f32::min);
        assert!(spread > 0.01, "grain must vary across positions, spread {spread}");
        // Pure black and white stay clean (film has no grain there).
        assert_eq!(apply_grain([0.0; 3], 3, 7, &adj), [0.0; 3]);
        assert_eq!(apply_grain([1.0; 3], 3, 7, &adj), [1.0; 3]);
    }

    #[test]
    fn grain_size_grows_the_cells() {
        // With coarse grain, adjacent pixels inside one 3px cell share a value.
        let coarse = Adjustments { grain: 1.0, grain_size: 1.0, ..Default::default() };
        let px = [0.5; 3];
        let a = apply_grain(px, 0, 0, &coarse);
        let b = apply_grain(px, 2, 2, &coarse);
        assert_eq!(a, b, "pixels in the same 3px cell share one grain value");
        // With fine grain those same coordinates are different cells.
        let fine = Adjustments { grain: 1.0, grain_size: 0.0, ..Default::default() };
        let c = apply_grain(px, 0, 0, &fine);
        let d = apply_grain(px, 2, 2, &fine);
        assert_ne!(c, d, "fine grain resolves per pixel");
    }

    #[test]
    fn identity_curve_is_a_no_op() {
        let adj = Adjustments::default();
        for x in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let out = apply_pixel(&adj, [x; 3]);
            assert!((out[0] - x).abs() < 1e-4, "identity curve moved {x} -> {}", out[0]);
        }
    }

    #[test]
    fn curve_lift_raises_midtones_and_pins_endpoints() {
        let curve = vec![
            CurvePoint { x: 0.0, y: 0.0 },
            CurvePoint { x: 0.5, y: 0.7 },
            CurvePoint { x: 1.0, y: 1.0 },
        ];
        let adj = Adjustments { curve, ..Default::default() };
        let mid = apply_pixel(&adj, [0.5; 3]);
        assert!(mid[0] > 0.65, "lifted midpoint should brighten mid-grey, got {}", mid[0]);
        assert!(apply_pixel(&adj, [0.0; 3])[0] < 1e-3, "black stays pinned");
        assert!((apply_pixel(&adj, [1.0; 3])[0] - 1.0).abs() < 1e-3, "white stays pinned");
    }

    #[test]
    fn curve_is_monotonic_through_a_steep_point() {
        // Even with a steep control point the mapping never decreases.
        let curve = vec![
            CurvePoint { x: 0.0, y: 0.0 },
            CurvePoint { x: 0.3, y: 0.05 },
            CurvePoint { x: 0.7, y: 0.95 },
            CurvePoint { x: 1.0, y: 1.0 },
        ];
        let mut prev = -1.0;
        for i in 0..=20 {
            let x = i as f32 / 20.0;
            let y = curve_eval(&curve, x);
            assert!(y >= prev - 1e-4, "curve dipped at x={x}: {y} < {prev}");
            prev = y;
        }
    }
}
