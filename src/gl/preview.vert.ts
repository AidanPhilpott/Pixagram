/** Vertex shader — draws a single full-canvas quad and passes UVs through. */
export const VERTEX_SHADER = /* glsl */ `
attribute vec2 aPosition; // clip-space, -1..1
attribute vec2 aUv;       // texture coords, 0..1
varying vec2 vUv;

void main() {
  vUv = aUv;
  gl_Position = vec4(aPosition, 0.0, 1.0);
}
`;
