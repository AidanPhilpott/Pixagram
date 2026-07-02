/** Pixel-buffer helpers shared by the live preview and the preset manager. */

/** Decode base64 little-endian u16 pixel data into a Uint16Array (LE targets). */
export function decodeU16(b64: string): Uint16Array {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return new Uint16Array(bytes.buffer);
}

/** Convert canvas RGBA u8 pixels to the renderer's RGB u16 layout (×257 spreads 0..255 to 0..65535). */
export function rgbaToU16(rgba: Uint8ClampedArray): Uint16Array {
  const out = new Uint16Array((rgba.length / 4) * 3);
  for (let i = 0, j = 0; i < rgba.length; i += 4) {
    out[j++] = rgba[i] * 257;
    out[j++] = rgba[i + 1] * 257;
    out[j++] = rgba[i + 2] * 257;
  }
  return out;
}
