/**
 * Fragment shader — the GPU half of the shared adjustment math.
 *
 * This MUST stay identical, step for step, to Rust `adjustments::apply_pixel`
 * (`src-tauri/src/adjustments.rs`). The canonical spec and the pipeline order
 * live in that Rust module. If you change the math there, change it here too.
 *
 * Pipeline: sRGB -> linear, exposure (2^EV), white balance (luminance-preserving
 * chroma shift), linear -> sRGB, tone regions (highlights/shadows/whites/blacks),
 * contrast (S-curve around mid-grey 0.5), saturation/vibrance, clip.
 */
export const FRAGMENT_SHADER = /* glsl */ `
precision highp float;

varying vec2 vUv;
uniform sampler2D uImage;
uniform float uExposure;    // EV (stops), from the shared Adjustments definition
uniform float uContrast;    // [-1, 1], from the shared Adjustments definition
uniform float uTemperature; // [-1, 1], white-balance temp (warm/cool)
uniform float uTint;        // [-1, 1], white-balance tint (green/magenta)
uniform float uHighlights;  // [-1, 1], tone region
uniform float uShadows;     // [-1, 1], tone region
uniform float uWhites;      // [-1, 1], tone region
uniform float uBlacks;      // [-1, 1], tone region
uniform float uSaturation;  // [-1, 1], chroma scale (uniform)
uniform float uVibrance;    // [-1, 1], chroma scale (weighted to muted colors)

// Full-scale chroma strengths — MUST match WB_*_STRENGTH in adjustments.rs.
const float WB_TEMP_STRENGTH = 0.3;
const float WB_TINT_STRENGTH = 0.3;
// Full-scale tone-region lift — MUST match TONE_STRENGTH in adjustments.rs.
const float TONE_STRENGTH = 0.2;

// Per-channel sRGB <-> linear, matching the Rust implementation exactly.
float srgbToLinear(float c) {
  return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4);
}
float linearToSrgb(float c) {
  return c <= 0.0031308 ? c * 12.92 : 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}
vec3 srgbToLinear(vec3 c) {
  return vec3(srgbToLinear(c.r), srgbToLinear(c.g), srgbToLinear(c.b));
}
vec3 linearToSrgb(vec3 c) {
  return vec3(linearToSrgb(c.r), linearToSrgb(c.g), linearToSrgb(c.b));
}

// Contrast S-curve on a perceptual [0,1] value, pivoting at mid-grey (0.5).
// Mirror of Rust apply_contrast() — edit both together.
float contrastCurve(float v, float c) {
  if (c >= 0.0) {
    return mix(v, smoothstep(0.0, 1.0, v), c);
  }
  return mix(v, 0.5 + (v - 0.5) * 0.5, -c);
}
vec3 contrastCurve(vec3 v, float c) {
  return vec3(contrastCurve(v.r, c), contrastCurve(v.g, c), contrastCurve(v.b, c));
}

// Tone-region shaping on a perceptual [0,1] value: highlights/shadows/whites/
// blacks each lift a smoothstep-weighted band. Mirror of Rust
// apply_tone_regions() — edit both together.
float toneRegions(float v) {
  float wHighlights = smoothstep(0.4, 0.9, v);
  float wShadows = 1.0 - smoothstep(0.1, 0.6, v);
  float wWhites = smoothstep(0.6, 1.0, v);
  float wBlacks = 1.0 - smoothstep(0.0, 0.4, v);
  float delta = uHighlights * wHighlights + uShadows * wShadows
              + uWhites * wWhites + uBlacks * wBlacks;
  return clamp(v + TONE_STRENGTH * delta, 0.0, 1.0);
}
vec3 toneRegions(vec3 v) {
  return vec3(toneRegions(v.r), toneRegions(v.g), toneRegions(v.b));
}

// Saturation/vibrance: scale chroma around the pixel's own luma. Mirror of Rust
// apply_saturation() — edit both together.
vec3 applySaturation(vec3 rgb, float saturation, float vibrance) {
  float luma = dot(rgb, vec3(0.2126, 0.7152, 0.0722));
  float pixelSat = max(max(rgb.r, rgb.g), rgb.b) - min(min(rgb.r, rgb.g), rgb.b);
  float factor = (1.0 + saturation) * (1.0 + vibrance * (1.0 - pixelSat));
  return clamp(mix(vec3(luma), rgb, factor), 0.0, 1.0);
}

// Luminance-preserving white balance on linear RGB.
// Mirror of Rust apply_white_balance() — edit both together.
vec3 applyWhiteBalance(vec3 lin, float temperature, float tint) {
  vec3 g = vec3(
    1.0 + WB_TEMP_STRENGTH * temperature,
    1.0 - WB_TINT_STRENGTH * tint,
    1.0 - WB_TEMP_STRENGTH * temperature
  );
  vec3 shifted = lin * g;
  const vec3 LUMA = vec3(0.2126, 0.7152, 0.0722);
  float before = dot(lin, LUMA);
  float after = dot(shifted, LUMA);
  float scale = after > 1e-6 ? before / after : 1.0;
  return shifted * scale;
}

void main() {
  vec4 src = texture2D(uImage, vUv);

  // Exposure, then white balance — both in linear light.
  float gain = pow(2.0, uExposure);
  vec3 lin = srgbToLinear(src.rgb) * gain;
  lin = applyWhiteBalance(lin, uTemperature, uTint);

  // Encode to the perceptual domain, then shape tone regions + contrast.
  vec3 v = clamp(linearToSrgb(max(lin, 0.0)), 0.0, 1.0);
  v = toneRegions(v);
  v = clamp(contrastCurve(v, uContrast), 0.0, 1.0);

  // Saturation / vibrance across all channels.
  vec3 outColor = applySaturation(v, uSaturation, uVibrance);

  gl_FragColor = vec4(outColor, src.a);
}
`;
