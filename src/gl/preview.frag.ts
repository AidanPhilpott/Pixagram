/**
 * Fragment shader — the GPU half of the shared adjustment math.
 *
 * This MUST stay identical, step for step, to Rust `adjustments::apply_pixel`
 * (`src-tauri/src/adjustments.rs`). The canonical spec and the pipeline order
 * live in that Rust module. If you change the math there, change it here too.
 *
 * Pipeline: sRGB -> linear, exposure (2^EV), linear -> sRGB, contrast (S-curve
 * around perceptual mid-grey 0.5), clip.
 */
export const FRAGMENT_SHADER = /* glsl */ `
precision highp float;

varying vec2 vUv;
uniform sampler2D uImage;
uniform float uExposure; // EV (stops), from the shared Adjustments definition
uniform float uContrast; // [-1, 1], from the shared Adjustments definition

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

void main() {
  vec4 src = texture2D(uImage, vUv);

  // Exposure in linear light, then encode to the perceptual domain.
  float gain = pow(2.0, uExposure);
  vec3 v = clamp(linearToSrgb(srgbToLinear(src.rgb) * gain), 0.0, 1.0);

  // Contrast S-curve on the perceptual value.
  vec3 outColor = clamp(contrastCurve(v, uContrast), 0.0, 1.0);

  gl_FragColor = vec4(outColor, src.a);
}
`;
