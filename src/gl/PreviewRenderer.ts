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
  private hasImage = false;
  private adjustments: Adjustments = defaultAdjustments();

  constructor(private canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl", {
      premultipliedAlpha: false,
      preserveDrawingBuffer: false,
    });
    if (!gl) throw new Error("WebGL is not available in this environment.");
    this.gl = gl;

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
    gl.uniform1i(gl.getUniformLocation(this.program, "uImage"), 0);
  }

  /** Replace the displayed image. Resizes the drawing buffer to match. */
  setImage(image: HTMLImageElement): void {
    const gl = this.gl;
    this.canvas.width = image.naturalWidth;
    this.canvas.height = image.naturalHeight;
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);

    if (this.texture) gl.deleteTexture(this.texture);
    this.texture = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image);

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
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }

  dispose(): void {
    const gl = this.gl;
    if (this.texture) gl.deleteTexture(this.texture);
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
