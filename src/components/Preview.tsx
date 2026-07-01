import { useEffect, useRef, useState } from "react";
import { PreviewRenderer } from "../gl/PreviewRenderer";
import type { PreviewPayload } from "../lib/ipc";
import type { Adjustments } from "../adjustments/definition";

interface PreviewProps {
  preview: PreviewPayload | null;
  adjustments: Adjustments;
  loading: boolean;
}

/** Decode base64 little-endian u16 pixel data into a Uint16Array (LE targets). */
function decodeU16(b64: string): Uint16Array {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return new Uint16Array(bytes.buffer);
}

/** WebGL live preview. Re-renders on adjustment change without re-decoding. */
export function Preview({ preview, adjustments, loading }: PreviewProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const rendererRef = useRef<PreviewRenderer | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Create the renderer once the canvas mounts.
  useEffect(() => {
    if (!canvasRef.current) return;
    try {
      rendererRef.current = new PreviewRenderer(canvasRef.current);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
    return () => {
      rendererRef.current?.dispose();
      rendererRef.current = null;
    };
  }, []);

  // Upload a new image whenever the developed preview changes.
  useEffect(() => {
    const renderer = rendererRef.current;
    if (!renderer || !preview) return;
    try {
      const data = decodeU16(preview.data);
      renderer.setImage(preview.width, preview.height, data);
      renderer.setAdjustments(adjustments);
    } catch {
      setError("Failed to decode preview pixel data.");
    }
    // adjustments intentionally omitted: re-uploading on every slider tick is wasteful.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preview]);

  // Cheap GPU-only update on any adjustment change.
  useEffect(() => {
    rendererRef.current?.setAdjustments(adjustments);
  }, [adjustments]);

  return (
    <div className="preview">
      {error && <div className="preview__error">{error}</div>}
      {!preview && !loading && !error && (
        <div className="preview__empty">
          <p>Select a photo to edit</p>
          <span>Decoded with X-Trans (Markesteijn) demosaicing</span>
        </div>
      )}
      {loading && <div className="preview__loading">Decoding RAF…</div>}
      <canvas
        ref={canvasRef}
        className="preview__canvas"
        style={{ display: preview && !loading ? "block" : "none" }}
      />
    </div>
  );
}
