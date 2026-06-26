/**
 * Fragment shader — the GPU half of the shared adjustment math.
 *
 * This MUST stay identical, step for step, to Rust `adjustments::apply_pixel`
 * (`src-tauri/src/adjustments.rs`). The canonical spec and the pipeline order
 * live in that Rust module. If you change the math there, change it here too.
 *
 * Phase 1 pipeline: sRGB -> linear, multiply by 2^EV, linear -> sRGB, clip.
 */
export const FRAGMENT_SHADER = /* glsl */ `
precision highp float;

varying vec2 vUv;
uniform sampler2D uImage;
uniform float uExposure; // EV (stops), from the shared Adjustments definition

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

void main() {
  vec4 src = texture2D(uImage, vUv);

  float gain = pow(2.0, uExposure);
  vec3 lin = srgbToLinear(src.rgb) * gain;
  vec3 outColor = clamp(linearToSrgb(lin), 0.0, 1.0);

  gl_FragColor = vec4(outColor, src.a);
}
`;
