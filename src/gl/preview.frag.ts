/**
 * Fragment shader — the GPU half of the shared adjustment math.
 *
 * This MUST stay identical, step for step, to Rust `adjustments::apply_pixel`
 * (`src-tauri/src/adjustments.rs`). The canonical spec and the pipeline order
 * live in that Rust module. If you change the math there, change it here too.
 *
 * Pipeline: sRGB -> linear, exposure (2^EV), white balance (luminance-preserving
 * chroma shift), linear -> sRGB, tone regions (highlights/shadows/whites/blacks),
 * contrast (S-curve around mid-grey 0.5), saturation/vibrance, HSL, clip.
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
uniform float uHslHue[8];   // per-band hue rotation [-1, 1]
uniform float uHslSat[8];   // per-band saturation [-1, 1]
uniform float uHslLum[8];   // per-band luminance [-1, 1]
uniform float uHslActive;   // 1.0 if any HSL band is non-neutral, else 0.0

// Full-scale chroma strengths — MUST match WB_*_STRENGTH in adjustments.rs.
const float WB_TEMP_STRENGTH = 0.3;
const float WB_TINT_STRENGTH = 0.3;
// Full-scale tone-region lift — MUST match TONE_STRENGTH in adjustments.rs.
const float TONE_STRENGTH = 0.2;
// HSL slider strengths — MUST match HSL_*_STRENGTH/DEG in adjustments.rs.
const float HSL_HUE_DEG = 30.0;
const float HSL_LUM_STRENGTH = 0.5;

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

// RGB <-> HSL (hue in degrees). Mirrors Rust rgb_to_hsl/hsl_to_rgb exactly.
vec3 rgb2hsl(vec3 c) {
  float maxc = max(max(c.r, c.g), c.b);
  float minc = min(min(c.r, c.g), c.b);
  float l = (maxc + minc) * 0.5;
  float d = maxc - minc;
  if (d <= 1e-6) return vec3(0.0, 0.0, l);
  float s = d / (1.0 - abs(2.0 * l - 1.0));
  float h;
  if (maxc == c.r) h = (c.g - c.b) / d;
  else if (maxc == c.g) h = (c.b - c.r) / d + 2.0;
  else h = (c.r - c.g) / d + 4.0;
  h = mod(h * 60.0, 360.0);
  return vec3(h, s, l);
}
vec3 hsl2rgb(float h, float s, float l) {
  float c = (1.0 - abs(2.0 * l - 1.0)) * s;
  float hp = h / 60.0;
  float x = c * (1.0 - abs(mod(hp, 2.0) - 1.0));
  vec3 rgb1;
  if (hp < 1.0) rgb1 = vec3(c, x, 0.0);
  else if (hp < 2.0) rgb1 = vec3(x, c, 0.0);
  else if (hp < 3.0) rgb1 = vec3(0.0, c, x);
  else if (hp < 4.0) rgb1 = vec3(0.0, x, c);
  else if (hp < 5.0) rgb1 = vec3(x, 0.0, c);
  else rgb1 = vec3(c, 0.0, x);
  return rgb1 + (l - c * 0.5);
}

// Per-hue HSL. Blends the two bands bracketing the pixel hue (constant-index
// access keeps this WebGL1-safe). Mirror of Rust apply_hsl() — edit both together.
vec3 applyHsl(vec3 rgb) {
  vec3 hsl = rgb2hsl(rgb);
  float h = hsl.x;
  float dh, ds, dl;
  if (h < 30.0)       { float t = h / 30.0;          dh = mix(uHslHue[0], uHslHue[1], t); ds = mix(uHslSat[0], uHslSat[1], t); dl = mix(uHslLum[0], uHslLum[1], t); }
  else if (h < 60.0)  { float t = (h - 30.0) / 30.0; dh = mix(uHslHue[1], uHslHue[2], t); ds = mix(uHslSat[1], uHslSat[2], t); dl = mix(uHslLum[1], uHslLum[2], t); }
  else if (h < 120.0) { float t = (h - 60.0) / 60.0; dh = mix(uHslHue[2], uHslHue[3], t); ds = mix(uHslSat[2], uHslSat[3], t); dl = mix(uHslLum[2], uHslLum[3], t); }
  else if (h < 180.0) { float t = (h - 120.0) / 60.0; dh = mix(uHslHue[3], uHslHue[4], t); ds = mix(uHslSat[3], uHslSat[4], t); dl = mix(uHslLum[3], uHslLum[4], t); }
  else if (h < 240.0) { float t = (h - 180.0) / 60.0; dh = mix(uHslHue[4], uHslHue[5], t); ds = mix(uHslSat[4], uHslSat[5], t); dl = mix(uHslLum[4], uHslLum[5], t); }
  else if (h < 270.0) { float t = (h - 240.0) / 30.0; dh = mix(uHslHue[5], uHslHue[6], t); ds = mix(uHslSat[5], uHslSat[6], t); dl = mix(uHslLum[5], uHslLum[6], t); }
  else if (h < 300.0) { float t = (h - 270.0) / 30.0; dh = mix(uHslHue[6], uHslHue[7], t); ds = mix(uHslSat[6], uHslSat[7], t); dl = mix(uHslLum[6], uHslLum[7], t); }
  else                { float t = (h - 300.0) / 60.0; dh = mix(uHslHue[7], uHslHue[0], t); ds = mix(uHslSat[7], uHslSat[0], t); dl = mix(uHslLum[7], uHslLum[0], t); }

  float colorMask = smoothstep(0.0, 0.15, hsl.y);
  float h2 = mod(h + dh * HSL_HUE_DEG * colorMask, 360.0);
  float s2 = clamp(hsl.y * (1.0 + ds * colorMask), 0.0, 1.0);
  float l2 = clamp(hsl.z + dl * HSL_LUM_STRENGTH * colorMask, 0.0, 1.0);
  return hsl2rgb(h2, s2, l2);
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

  // Per-hue HSL (skipped entirely when no band is touched).
  if (uHslActive > 0.5) {
    outColor = applyHsl(outColor);
  }

  gl_FragColor = vec4(outColor, src.a);
}
`;
