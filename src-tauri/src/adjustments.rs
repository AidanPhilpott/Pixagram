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
//! Phase 1 has exactly one operation — exposure — applied in **linear light**:
//!
//! 1. Decode the stored pixel from sRGB gamma to linear.
//! 2. Multiply linear RGB by the exposure gain `2^EV`.
//! 3. Re-encode linear back to sRGB gamma and clip to [0, 1].
//!
//! Later phases insert contrast, white balance, tone curve, HSL, etc. into this
//! same linear-light pipeline, all driven by additional fields on `Adjustments`.

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
}

impl Default for Adjustments {
    fn default() -> Self {
        Self { exposure: 0.0 }
    }
}

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

/// Apply the full adjustment stack to a single sRGB-encoded RGB pixel.
///
/// Input and output channels are normalized to `0.0..=1.0`. This is the Rust
/// half of the shared math; the GLSL fragment shader implements the same steps
/// in the same order. **If you edit this, edit the shader too.**
#[inline]
pub fn apply_pixel(adj: &Adjustments, rgb: [f32; 3]) -> [f32; 3] {
    let gain = 2.0_f32.powf(adj.exposure);
    let mut out = [0.0_f32; 3];
    for i in 0..3 {
        let lin = srgb_to_linear(rgb[i]) * gain;
        out[i] = linear_to_srgb(lin).clamp(0.0, 1.0);
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
        let adj = Adjustments { exposure: 1.0 };
        let mid = 0.5_f32;
        let out = apply_pixel(&adj, [mid, mid, mid]);
        let expected = linear_to_srgb((srgb_to_linear(mid) * 2.0).min(1.0));
        assert!((out[0] - expected).abs() < 1e-4);
    }
}
