import { useEffect, useRef, useState } from "react";
import { PreviewRenderer } from "../gl/PreviewRenderer";
import type { PreviewPayload } from "../lib/ipc";

interface PreviewProps {
  preview: PreviewPayload | null;
  exposure: number;
  loading: boolean;
}

/** WebGL live preview. Re-renders on exposure change without re-decoding. */
export function Preview({ preview, exposure, loading }: PreviewProps) {
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
    const img = new Image();
    img.onload = () => {
      renderer.setImage(img);
      renderer.setExposure(exposure);
    };
    img.onerror = () => setError("Failed to load decoded preview image.");
    img.src = preview.dataUrl;
    // exposure intentionally omitted: re-uploading on every slider tick is wasteful.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preview]);

  // Cheap GPU-only update on exposure change.
  useEffect(() => {
    rendererRef.current?.setExposure(exposure);
  }, [exposure]);

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
