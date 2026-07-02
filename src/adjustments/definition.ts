/**
 * The shared adjustment definition — TypeScript side.
 *
 * The TYPES (`Adjustments`, `HslBand`, `CurvePoint`) are GENERATED from the Rust
 * structs in `src-tauri/src/adjustments.rs` by ts-rs — see `./generated/`. They
 * can't drift from Rust by construction (constraint #1). Regenerate them with
 * `npm run gen:types` after changing the Rust structs.
 *
 * This file adds only the runtime helpers TS needs on top of those types
 * (defaults, band metadata, UI ranges). The matching pixel math lives in Rust
 * `apply_pixel` and the GLSL shader (`src/gl/preview.frag.ts`).
 */
import type { Adjustments } from "./generated/Adjustments";
import type { CurvePoint } from "./generated/CurvePoint";
import type { HslBand } from "./generated/HslBand";

export type { Adjustments, CurvePoint, HslBand };

/**
 * The 8 HSL color bands, in order. `hue` is the band centre in degrees, used
 * for the swatch colour and label — the per-pixel band math lives in the Rust
 * `hsl_band_adjust` / GLSL `applyHsl` (segment thresholds must match). The count
 * and order must line up with the Rust band centers.
 */
export const HSL_BANDS = [
  { name: "Red", hue: 0 },
  { name: "Orange", hue: 30 },
  { name: "Yellow", hue: 60 },
  { name: "Green", hue: 120 },
  { name: "Aqua", hue: 180 },
  { name: "Blue", hue: 240 },
  { name: "Purple", hue: 270 },
  { name: "Magenta", hue: 300 },
] as const;

/** The neutral tone curve — the identity diagonal. */
export function identityCurve(): CurvePoint[] {
  return [
    { x: 0, y: 0 },
    { x: 1, y: 1 },
  ];
}

/** The neutral stack — equivalent to Rust's `Adjustments::default()`. */
export function defaultAdjustments(): Adjustments {
  return {
    exposure: 0,
    contrast: 0,
    temperature: 0,
    tint: 0,
    highlights: 0,
    shadows: 0,
    whites: 0,
    blacks: 0,
    saturation: 0,
    vibrance: 0,
    hsl: HSL_BANDS.map(() => ({ hue: 0, sat: 0, lum: 0 })),
    curve: identityCurve(),
    colorChrome: 0,
    colorChromeBlue: 0,
    grain: 0,
    grainSize: 0,
    rotation: 0,
  };
}

/**
 * True when a stack is indistinguishable from the neutral defaults — i.e. the
 * photo has no persisting edit. Drives the grid's "edited" badge. Curve points
 * all on the diagonal count as identity (a monotonic spline through collinear
 * points IS the straight line); rotation counts as an edit.
 */
export function isNeutralAdjustments(a: Adjustments): boolean {
  return (
    a.exposure === 0 &&
    a.contrast === 0 &&
    a.temperature === 0 &&
    a.tint === 0 &&
    a.highlights === 0 &&
    a.shadows === 0 &&
    a.whites === 0 &&
    a.blacks === 0 &&
    a.saturation === 0 &&
    a.vibrance === 0 &&
    a.colorChrome === 0 &&
    a.colorChromeBlue === 0 &&
    a.grain === 0 &&
    a.grainSize === 0 &&
    a.rotation === 0 &&
    a.hsl.every((b) => b.hue === 0 && b.sat === 0 && b.lum === 0) &&
    a.curve.every((p) => p.x === p.y)
  );
}

/** UI bounds for the exposure slider (stops). Tunable constant, not architecture. */
export const EXPOSURE_RANGE = { min: -5, max: 5, step: 0.01 } as const;

/** UI bounds for the contrast slider (stored in [-1, 1]). */
export const CONTRAST_RANGE = { min: -1, max: 1, step: 0.01 } as const;

/** UI bounds for the white-balance sliders (stored in [-1, 1]). */
export const WB_RANGE = { min: -1, max: 1, step: 0.01 } as const;

/** UI bounds for the tone-region sliders (stored in [-1, 1]). */
export const TONE_RANGE = { min: -1, max: 1, step: 0.01 } as const;

/** UI bounds for the saturation/vibrance sliders (stored in [-1, 1]). */
export const COLOR_RANGE = { min: -1, max: 1, step: 0.01 } as const;

/** UI bounds for the per-band HSL sliders (stored in [-1, 1]). */
export const HSL_RANGE = { min: -1, max: 1, step: 0.01 } as const;

/** UI bounds for the effect sliders (grain, color chrome — stored in [0, 1]). */
export const EFFECT_RANGE = { min: 0, max: 1, step: 0.01 } as const;
