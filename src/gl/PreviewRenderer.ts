/**
 * Minimal WebGL renderer for the live editing preview.
 *
 * Uploads a developed sRGB preview image to a texture and re-renders it through
 * the shared adjustment shader whenever exposure changes. Heavy pixel work
 * (decode/demosaic) is done in Rust; this just composites on the GPU.
 */
import { VERTEX_SHADER } from "./preview.vert";
import { FRAGMENT_SHADER } from "./preview.frag";
import { defaultAdjustments, type Adjustments } from "../adjustments/definition";
import { bakeCurveLut, isIdentityCurve } from "../adjustments/curve";

export class PreviewRenderer {
  private gl: WebGLRenderingContext;
  private program: WebGLProgram;
  private texture: WebGLTexture | null = null;
  private uExposure: WebGLUniformLocation | null;
  private uContrast: WebGLUniformLocation | null;
  private uTemperature: WebGLUniformLocation | null;
  private uTint: WebGLUniformLocation | null;
  private uHighlights: WebGLUniformLocation | null;
  private uShadows: WebGLUniformLocation | null;
  private uWhites: WebGLUniformLocation | null;
  private uBlacks: WebGLUniformLocation | null;
  private uSaturation: WebGLUniformLocation | null;
  private uVibrance: WebGLUniformLocation | null;
  private uHslHue: WebGLUniformLocation | null;
  private uHslSat: WebGLUniformLocation | null;
  private uHslLum: WebGLUniformLocation | null;
  private uHslActive: WebGLUniformLocation | null;
  private uCurveActive: WebGLUniformLocation | null;
  private curveTexture: WebGLTexture | null = null;
  /** True when float textures + linear filtering are available (16-bit preview). */
  private floatLinear = false;
  // Reused scratch buffers so per-frame uniform uploads don't allocate.
  private hslHue = new Float32Array(8);
  private hslSat = new Float32Array(8);
  private hslLum = new Float32Array(8);
  private hasImage = false;
  private adjustments: Adjustments = defaultAdjustments();

  constructor(private canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl", {
      premultipliedAlpha: false,
      preserveDrawingBuffer: false,
    });
    if (!gl) throw new Error("WebGL is not available in this environment.");
    this.gl = gl;

    // 16-bit preview needs float textures + linear filtering of them. Ubiquitous
    // on desktop; if absent we fall back to an 8-bit upload in setImage.
    const floatExt = gl.getExtension("OES_texture_float");
    const floatLinearExt = gl.getExtension("OES_texture_float_linear");
    this.floatLinear = !!(floatExt && floatLinearExt);

    this.program = this.buildProgram(VERTEX_SHADER, FRAGMENT_SHADER);
    gl.useProgram(this.program);

    // Full-canvas quad: two triangles. Interleaved [x, y, u, v].
    const quad = new Float32Array([
      -1, -1, 0, 0,
      1, -1, 1, 0,
      -1, 1, 0, 1,
      -1, 1, 0, 1,
      1, -1, 1, 0,
      1, 1, 1, 1,
    ]);
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, quad, gl.STATIC_DRAW);

    const stride = 4 * Float32Array.BYTES_PER_ELEMENT;
    const aPosition = gl.getAttribLocation(this.program, "aPosition");
    gl.enableVertexAttribArray(aPosition);
    gl.vertexAttribPointer(aPosition, 2, gl.FLOAT, false, stride, 0);

    const aUv = gl.getAttribLocation(this.program, "aUv");
    gl.enableVertexAttribArray(aUv);
    gl.vertexAttribPointer(aUv, 2, gl.FLOAT, false, stride, 2 * Float32Array.BYTES_PER_ELEMENT);

    this.uExposure = gl.getUniformLocation(this.program, "uExposure");
    this.uContrast = gl.getUniformLocation(this.program, "uContrast");
    this.uTemperature = gl.getUniformLocation(this.program, "uTemperature");
    this.uTint = gl.getUniformLocation(this.program, "uTint");
    this.uHighlights = gl.getUniformLocation(this.program, "uHighlights");
    this.uShadows = gl.getUniformLocation(this.program, "uShadows");
    this.uWhites = gl.getUniformLocation(this.program, "uWhites");
    this.uBlacks = gl.getUniformLocation(this.program, "uBlacks");
    this.uSaturation = gl.getUniformLocation(this.program, "uSaturation");
    this.uVibrance = gl.getUniformLocation(this.program, "uVibrance");
    this.uHslHue = gl.getUniformLocation(this.program, "uHslHue");
    this.uHslSat = gl.getUniformLocation(this.program, "uHslSat");
    this.uHslLum = gl.getUniformLocation(this.program, "uHslLum");
    this.uHslActive = gl.getUniformLocation(this.program, "uHslActive");
    this.uCurveActive = gl.getUniformLocation(this.program, "uCurveActive");

    // Texture units: 0 = image, 1 = tone-curve LUT.
    gl.uniform1i(gl.getUniformLocation(this.program, "uImage"), 0);
    gl.uniform1i(gl.getUniformLocation(this.program, "uCurveLut"), 1);

    // 256x1 LUT texture; linear-filtered so the shader interpolates between entries.
    // Seed with an identity ramp so the texture is always complete (the shader
    // skips sampling it when the curve is inactive, but it must still be valid).
    this.curveTexture = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, this.curveTexture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    const identityRamp = new Uint8Array(256);
    for (let i = 0; i < 256; i++) identityRamp[i] = i;
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.LUMINANCE, 256, 1, 0, gl.LUMINANCE, gl.UNSIGNED_BYTE, identityRamp);
  }

  /**
   * Upload a developed preview from raw 16-bit sRGB-encoded pixels (row 0 at top,
   * length `width*height*3`). Uses a float texture when available so stacked tone
   * ops keep headroom; otherwise falls back to 8-bit. Resizes the buffer to match.
   */
  setImage(width: number, height: number, data: Uint16Array): void {
    const gl = this.gl;
    this.canvas.width = width;
    this.canvas.height = height;
    gl.viewport(0, 0, width, height);

    if (this.texture) gl.deleteTexture(this.texture);
    this.texture = gl.createTexture();
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    // Flip Y in the buffer (UNPACK_FLIP_Y doesn't apply to ArrayBufferView uploads).
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);

    const rowLen = width * 3;
    if (this.floatLinear) {
      const f = new Float32Array(width * height * 3);
      for (let oy = 0; oy < height; oy++) {
        const src = oy * rowLen;
        const dst = (height - 1 - oy) * rowLen;
        for (let i = 0; i < rowLen; i++) f[dst + i] = data[src + i] / 65535;
      }
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, width, height, 0, gl.RGB, gl.FLOAT, f);
    } else {
      const b = new Uint8Array(width * height * 3);
      for (let oy = 0; oy < height; oy++) {
        const src = oy * rowLen;
        const dst = (height - 1 - oy) * rowLen;
        for (let i = 0; i < rowLen; i++) b[dst + i] = data[src + i] >> 8;
      }
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, width, height, 0, gl.RGB, gl.UNSIGNED_BYTE, b);
    }

    this.hasImage = true;
    this.render();
  }

  /** Push the whole adjustment stack into the shader uniforms and redraw. */
  setAdjustments(adj: Adjustments): void {
    this.adjustments = adj;
    this.render();
  }

  render(): void {
    const gl = this.gl;
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    if (!this.hasImage) return;
    gl.useProgram(this.program);
    gl.uniform1f(this.uExposure, this.adjustments.exposure);
    gl.uniform1f(this.uContrast, this.adjustments.contrast);
    gl.uniform1f(this.uTemperature, this.adjustments.temperature);
    gl.uniform1f(this.uTint, this.adjustments.tint);
    gl.uniform1f(this.uHighlights, this.adjustments.highlights);
    gl.uniform1f(this.uShadows, this.adjustments.shadows);
    gl.uniform1f(this.uWhites, this.adjustments.whites);
    gl.uniform1f(this.uBlacks, this.adjustments.blacks);
    gl.uniform1f(this.uSaturation, this.adjustments.saturation);
    gl.uniform1f(this.uVibrance, this.adjustments.vibrance);

    let hslActive = 0;
    const hsl = this.adjustments.hsl;
    for (let i = 0; i < 8; i++) {
      const band = hsl[i] ?? { hue: 0, sat: 0, lum: 0 };
      this.hslHue[i] = band.hue;
      this.hslSat[i] = band.sat;
      this.hslLum[i] = band.lum;
      if (band.hue !== 0 || band.sat !== 0 || band.lum !== 0) hslActive = 1;
    }
    gl.uniform1fv(this.uHslHue, this.hslHue);
    gl.uniform1fv(this.uHslSat, this.hslSat);
    gl.uniform1fv(this.uHslLum, this.hslLum);
    gl.uniform1f(this.uHslActive, hslActive);

    // Tone curve on unit 1: keep it bound (seeded identity ramp) so the sampler is
    // always complete; re-bake + upload the LUT only when the curve is non-identity.
    const curve = this.adjustments.curve;
    const curveActive = curve && !isIdentityCurve(curve) ? 1 : 0;
    gl.uniform1f(this.uCurveActive, curveActive);
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.curveTexture);
    if (curveActive) {
      const lut = bakeCurveLut(curve);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.LUMINANCE, lut.length, 1, 0, gl.LUMINANCE, gl.UNSIGNED_BYTE, lut);
    }

    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }

  dispose(): void {
    const gl = this.gl;
    if (this.texture) gl.deleteTexture(this.texture);
    if (this.curveTexture) gl.deleteTexture(this.curveTexture);
    gl.deleteProgram(this.program);
  }

  private buildProgram(vsSource: string, fsSource: string): WebGLProgram {
    const gl = this.gl;
    const vs = this.compile(gl.VERTEX_SHADER, vsSource);
    const fs = this.compile(gl.FRAGMENT_SHADER, fsSource);
    const program = gl.createProgram();
    if (!program) throw new Error("Failed to create WebGL program.");
    gl.attachShader(program, vs);
    gl.attachShader(program, fs);
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      const log = gl.getProgramInfoLog(program);
      throw new Error(`WebGL program link failed: ${log}`);
    }
    gl.deleteShader(vs);
    gl.deleteShader(fs);
    return program;
  }

  private compile(type: number, source: string): WebGLShader {
    const gl = this.gl;
    const shader = gl.createShader(type);
    if (!shader) throw new Error("Failed to create shader.");
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      const log = gl.getShaderInfoLog(shader);
      gl.deleteShader(shader);
      throw new Error(`Shader compile failed: ${log}`);
    }
    return shader;
  }
}
