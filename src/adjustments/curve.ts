/**
 * Tone-curve math — the TypeScript half of the shared spline.
 *
 * `evalCurve` MUST stay identical to Rust `curve_eval` in
 * `src-tauri/src/adjustments.rs` (same monotonic-cubic / Fritsch–Carlson spline).
 * The GPU preview doesn't re-implement the spline: it samples a 256-entry LUT
 * baked here by `bakeCurveLut`, while Rust export evaluates the spline directly.
 * Same curve shape → preview matches export.
 */
import type { CurvePoint } from "./definition";

/** Largest point count the Rust evaluator handles (keep in sync with MAX_CURVE_POINTS). */
const MAX_CURVE_POINTS = 16;

/** Evaluate the tone curve at `x` (perceptual, [0,1]). Points sorted by x, ≥2 entries. */
export function evalCurve(points: CurvePoint[], x: number): number {
  const n = Math.min(points.length, MAX_CURVE_POINTS);
  if (n < 2) return n === 1 ? points[0].y : clamp01(x);
  if (x <= points[0].x) return points[0].y;
  if (x >= points[n - 1].x) return points[n - 1].y;

  // Secant slopes.
  const d = new Array<number>(n - 1);
  for (let i = 0; i < n - 1; i++) {
    const dx = points[i + 1].x - points[i].x;
    d[i] = dx > 1e-9 ? (points[i + 1].y - points[i].y) / dx : 0;
  }

  // Initial tangents, then Fritsch–Carlson monotonicity clamp.
  const m = new Array<number>(n);
  m[0] = d[0];
  m[n - 1] = d[n - 2];
  for (let i = 1; i < n - 1; i++) m[i] = (d[i - 1] + d[i]) * 0.5;
  for (let i = 0; i < n - 1; i++) {
    if (Math.abs(d[i]) < 1e-9) {
      m[i] = 0;
      m[i + 1] = 0;
    } else {
      const a = m[i] / d[i];
      const b = m[i + 1] / d[i];
      const sq = a * a + b * b;
      if (sq > 9) {
        const t = 3 / Math.sqrt(sq);
        m[i] = t * a * d[i];
        m[i + 1] = t * b * d[i];
      }
    }
  }

  // Hermite-interpolate the segment containing x.
  let k = 0;
  while (k < n - 2 && x >= points[k + 1].x) k++;
  const h = points[k + 1].x - points[k].x;
  const t = (x - points[k].x) / h;
  const t2 = t * t;
  const t3 = t2 * t;
  const h00 = 2 * t3 - 3 * t2 + 1;
  const h10 = t3 - 2 * t2 + t;
  const h01 = -2 * t3 + 3 * t2;
  const h11 = t3 - t2;
  const y = h00 * points[k].y + h10 * h * m[k] + h01 * points[k + 1].y + h11 * h * m[k + 1];
  return clamp01(y);
}

/** Bake the curve into a 256-entry 8-bit LUT the shader samples as a texture. */
export function bakeCurveLut(points: CurvePoint[], size = 256): Uint8Array {
  const lut = new Uint8Array(size);
  for (let i = 0; i < size; i++) {
    const y = evalCurve(points, i / (size - 1));
    lut[i] = Math.round(clamp01(y) * 255);
  }
  return lut;
}

/** The neutral curve — the identity diagonal, a no-op. */
export function isIdentityCurve(points: CurvePoint[]): boolean {
  return (
    points.length === 2 &&
    points[0].x === 0 &&
    points[0].y === 0 &&
    points[1].x === 1 &&
    points[1].y === 1
  );
}

function clamp01(v: number): number {
  return v < 0 ? 0 : v > 1 ? 1 : v;
}
