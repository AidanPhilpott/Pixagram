/**
 * The shared adjustment definition — TypeScript MIRROR.
 *
 * The canonical source of truth is the Rust struct in
 * `src-tauri/src/adjustments.rs`. This file MUST stay field- and
 * default-compatible with it: same field names (camelCase on both sides via
 * serde `rename_all`), same defaults, same numeric semantics.
 *
 * This object is sent to Rust verbatim on export and fed to the GPU preview, so
 * the two pipelines never diverge. The matching pixel math lives in the GLSL
 * fragment shader (`src/gl/preview.frag.ts`) and in Rust `apply_pixel`.
 *
 * Phase 6+: replace this hand-written mirror with `ts-rs`-generated types so
 * divergence becomes impossible.
 */

/** One color band of the HSL panel. Mirror of Rust `HslBand`. */
export interface HslBand {
  /** Hue rotation in [-1, 1]. 0 = unchanged. */
  hue: number;
  /** Saturation in [-1, 1]. 0 = unchanged. */
  sat: number;
  /** Luminance in [-1, 1]. 0 = unchanged. */
  lum: number;
}

/**
 * The 8 HSL color bands, in order. `hue` is the band centre in degrees, used
 * for the swatch colour and label — the per-pixel band math lives in the Rust
 * `hsl_band_adjust` / GLSL `applyHsl` (segment thresholds must match).
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

/** A complete non-destructive adjustment stack for one photo. */
export interface Adjustments {
  /** Exposure compensation in stops (EV). 0 = unchanged. UI range ~[-5, +5]. */
  exposure: number;
  /** Contrast in [-1, 1]. 0 = unchanged. Shown to the user as [-100, +100]. */
  contrast: number;
  /** WB temperature in [-1, 1]. 0 = neutral; + warms, - cools. UI [-100, +100]. */
  temperature: number;
  /** WB tint in [-1, 1]. 0 = neutral; + magenta, - green. UI [-100, +100]. */
  tint: number;
  /** Highlights in [-1, 1]. 0 = unchanged; + brightens, - recovers. UI [-100, +100]. */
  highlights: number;
  /** Shadows in [-1, 1]. 0 = unchanged; + lifts, - deepens. UI [-100, +100]. */
  shadows: number;
  /** Whites in [-1, 1]. 0 = unchanged; + brightens the near-white end. UI [-100, +100]. */
  whites: number;
  /** Blacks in [-1, 1]. 0 = unchanged; + lifts / - crushes near-black. UI [-100, +100]. */
  blacks: number;
  /** Saturation in [-1, 1]. 0 = unchanged; -1 greyscale, +1 doubles chroma. UI [-100, +100]. */
  saturation: number;
  /** Vibrance in [-1, 1]. 0 = unchanged; like saturation, weighted to muted colors. UI [-100, +100]. */
  vibrance: number;
  /** Per-hue HSL adjustments, one per `HSL_BANDS` entry (length 8). */
  hsl: HslBand[];
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
  };
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
