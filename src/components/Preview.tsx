import { useCallback, useEffect, useRef, useState } from "react";
import { PreviewRenderer } from "../gl/PreviewRenderer";
import type { PreviewPayload } from "../lib/ipc";
import type { Adjustments } from "../adjustments/definition";

interface PreviewProps {
  preview: PreviewPayload | null;
  adjustments: Adjustments;
  loading: boolean;
  /** Rotate the selected photo by ±1 clockwise quarter-turn. */
  onRotate: (quarterTurns: 1 | -1) => void;
}

/**
 * Zoom/pan view state. `null` means Fit — the canvas sits in its natural
 * CSS-fitted layout with no transform. When set, `scale` is relative to that
 * fitted size and `tx`/`ty` translate the canvas from its centered position
 * (CSS `translate(tx, ty) scale(scale)` around the canvas center).
 */
type View = { scale: number; tx: number; ty: number };

/** Zoom ceiling as a fraction of native preview pixels (800%). */
const MAX_NATIVE_ZOOM = 8;
/** Wheel sensitivity: one notch (deltaY ≈ 100) ≈ ×1.2. */
const WHEEL_ZOOM_RATE = 0.0018;

/** Decode base64 little-endian u16 pixel data into a Uint16Array (LE targets). */
function decodeU16(b64: string): Uint16Array {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return new Uint16Array(bytes.buffer);
}

/** WebGL live preview. Re-renders on adjustment change without re-decoding. */
export function Preview({ preview, adjustments, loading, onRotate }: PreviewProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const rendererRef = useRef<PreviewRenderer | null>(null);
  const [error, setError] = useState<string | null>(null);

  // ── Zoom / pan ──
  const [view, setView] = useState<View | null>(null);
  // Live mirror for wheel/drag handlers (native listeners see stale state).
  const viewRef = useRef<View | null>(null);
  viewRef.current = view;
  const dragRef = useRef<{ pointerId: number; x: number; y: number } | null>(null);

  /** Scale that shows one preview pixel per CSS pixel, relative to Fit. */
  const nativeScale = useCallback(() => {
    const canvas = canvasRef.current;
    // clientWidth is the CSS-fitted layout size — transforms don't affect it.
    if (!canvas || canvas.clientWidth === 0) return 1;
    return canvas.width / canvas.clientWidth;
  }, []);

  /**
   * Zoom by `factor` keeping the container point `(cx, cy)` (viewport coords,
   * defaulting to the container center) visually fixed. The transform maps a
   * content offset `p` from the canvas center to `center + t + scale·p`, so
   * anchoring the cursor solves to `t' = c − center − (s'/s)·(c − center − t)`.
   */
  const zoomBy = useCallback(
    (factor: number, cx?: number, cy?: number) => {
      const container = containerRef.current;
      if (!container || !canvasRef.current) return;
      const rect = container.getBoundingClientRect();
      const centerX = rect.left + rect.width / 2;
      const centerY = rect.top + rect.height / 2;
      const anchorX = cx ?? centerX;
      const anchorY = cy ?? centerY;

      const cur = viewRef.current ?? { scale: 1, tx: 0, ty: 0 };
      const maxScale = Math.max(1, MAX_NATIVE_ZOOM / nativeScale());
      const next = Math.min(maxScale, cur.scale * factor);
      if (next <= 1.001) {
        setView(null); // zoomed all the way back out — snap to Fit
        return;
      }
      const ratio = next / cur.scale;
      setView({
        scale: next,
        tx: anchorX - centerX - ratio * (anchorX - centerX - cur.tx),
        ty: anchorY - centerY - ratio * (anchorY - centerY - cur.ty),
      });
    },
    [nativeScale]
  );

  /** Toggle Fit ↔ 100% (one preview pixel per CSS pixel), like Lightroom. */
  const toggleFitNative = useCallback(() => {
    if (viewRef.current) setView(null);
    else setView({ scale: Math.max(1, nativeScale()), tx: 0, ty: 0 });
  }, [nativeScale]);

  // Reset to Fit when the photo or its orientation changes — the canvas
  // aspect just swapped under the transform, so any old pan is meaningless.
  useEffect(() => {
    setView(null);
  }, [preview, adjustments.rotation]);

  // Wheel zoom, attached natively so preventDefault works (React's wheel
  // listeners are passive) and the page never scroll-bounces.
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      zoomBy(Math.exp(-e.deltaY * WHEEL_ZOOM_RATE), e.clientX, e.clientY);
    };
    container.addEventListener("wheel", onWheel, { passive: false });
    return () => container.removeEventListener("wheel", onWheel);
  }, [zoomBy]);

  // Drag to pan while zoomed in.
  const onPointerDown = useCallback((e: React.PointerEvent) => {
    if (!viewRef.current || e.button !== 0) return;
    dragRef.current = { pointerId: e.pointerId, x: e.clientX, y: e.clientY };
    (e.target as Element).setPointerCapture(e.pointerId);
  }, []);
  const onPointerMove = useCallback((e: React.PointerEvent) => {
    const drag = dragRef.current;
    const cur = viewRef.current;
    if (!drag || drag.pointerId !== e.pointerId || !cur) return;
    const dx = e.clientX - drag.x;
    const dy = e.clientY - drag.y;
    dragRef.current = { pointerId: e.pointerId, x: e.clientX, y: e.clientY };
    setView({ ...cur, tx: cur.tx + dx, ty: cur.ty + dy });
  }, []);
  const onPointerUp = useCallback((e: React.PointerEvent) => {
    if (dragRef.current?.pointerId === e.pointerId) dragRef.current = null;
  }, []);

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

  // Readout: percentage of native preview pixels currently on screen.
  const zoomPercent = view
    ? Math.round((view.scale / nativeScale()) * 100)
    : null;

  return (
    <div className="preview" ref={containerRef}>
      {error && <div className="preview__error">{error}</div>}
      {!preview && !loading && !error && (
        <div className="preview__empty">
          <p>Select a photo to edit</p>
          <span>RAF, NEF, CR2/CR3, ARW, DNG, ORF, RW2, PEF</span>
        </div>
      )}
      {loading && <div className="preview__loading">Decoding RAW…</div>}
      <canvas
        ref={canvasRef}
        className="preview__canvas"
        style={{
          display: preview && !loading ? "block" : "none",
          transform: view
            ? `translate(${view.tx}px, ${view.ty}px) scale(${view.scale})`
            : undefined,
          cursor: view ? "grab" : undefined,
        }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
        onDoubleClick={toggleFitNative}
      />
      {preview && !loading && (
        <div className="stage__toolbar">
          <button className="stage__tool" title="Zoom out" onClick={() => zoomBy(1 / 1.5)}>
            −
          </button>
          <button
            className="stage__tool stage__zoom"
            title="Toggle Fit / 100% (double-click the photo)"
            onClick={toggleFitNative}
          >
            {zoomPercent === null ? "Fit" : `${zoomPercent}%`}
          </button>
          <button className="stage__tool" title="Zoom in" onClick={() => zoomBy(1.5)}>
            +
          </button>
          <span className="stage__sep" />
          <button
            className="stage__tool"
            title="Rotate left  ( [ )"
            onClick={() => onRotate(-1)}
          >
            ⟲
          </button>
          <button
            className="stage__tool"
            title="Rotate right  ( ] )"
            onClick={() => onRotate(1)}
          >
            ⟳
          </button>
        </div>
      )}
    </div>
  );
}
