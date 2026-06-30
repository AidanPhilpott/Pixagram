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

/** A complete non-destructive adjustment stack for one photo. */
export interface Adjustments {
  /** Exposure compensation in stops (EV). 0 = unchanged. UI range ~[-5, +5]. */
  exposure: number;
  /** Contrast in [-1, 1]. 0 = unchanged. Shown to the user as [-100, +100]. */
  contrast: number;
}

/** The neutral stack — equivalent to Rust's `Adjustments::default()`. */
export function defaultAdjustments(): Adjustments {
  return { exposure: 0, contrast: 0 };
}

/** UI bounds for the exposure slider (stops). Tunable constant, not architecture. */
export const EXPOSURE_RANGE = { min: -5, max: 5, step: 0.01 } as const;

/** UI bounds for the contrast slider (stored in [-1, 1]). */
export const CONTRAST_RANGE = { min: -1, max: 1, step: 0.01 } as const;
