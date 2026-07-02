import { useRef } from "react";
import type { PointerEvent } from "react";
import { MIN_CROP_SIZE, type CropRect } from "../adjustments/definition";

/** Which part of the crop rect a drag grabs. */
type Handle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "move";

interface CropOverlayProps {
  /** The canvas's box in stage-container coordinates (CSS px). */
  box: { left: number; top: number; width: number; height: number };
  /** Normalized crop rect (top-left origin) in the straightened frame. */
  crop: CropRect;
  onChange: (crop: CropRect) => void;
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/**
 * Interactive crop rectangle drawn over the (uncropped, straightened) preview
 * while crop mode is active. Pure UI — every change flows out as a normalized
 * `CropRect` through `onChange` (which App routes through the undo-coalescing
 * `applyEdit`). Corner/edge handles resize, dragging inside moves, a thirds
 * grid aids composition, and the surround is dimmed.
 */
export function CropOverlay({ box, crop, onChange }: CropOverlayProps) {
  const dragRef = useRef<{
    handle: Handle;
    pointerId: number;
    startX: number;
    startY: number;
    start: CropRect;
  } | null>(null);

  const beginDrag = (handle: Handle) => (e: PointerEvent) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    e.preventDefault();
    dragRef.current = {
      handle,
      pointerId: e.pointerId,
      startX: e.clientX,
      startY: e.clientY,
      start: { ...crop },
    };
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: PointerEvent) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== e.pointerId) return;
    const du = (e.clientX - drag.startX) / box.width;
    const dv = (e.clientY - drag.startY) / box.height;
    const s = drag.start;
    let { x, y, width, height } = s;

    if (drag.handle === "move") {
      x = clamp(s.x + du, 0, 1 - s.width);
      y = clamp(s.y + dv, 0, 1 - s.height);
    } else {
      const h = drag.handle;
      if (h.includes("w")) {
        x = clamp(s.x + du, 0, s.x + s.width - MIN_CROP_SIZE);
        width = s.x + s.width - x;
      }
      if (h.includes("e")) {
        width = clamp(s.width + du, MIN_CROP_SIZE, 1 - s.x);
      }
      if (h.includes("n")) {
        y = clamp(s.y + dv, 0, s.y + s.height - MIN_CROP_SIZE);
        height = s.y + s.height - y;
      }
      if (h.includes("s")) {
        height = clamp(s.height + dv, MIN_CROP_SIZE, 1 - s.y);
      }
    }
    onChange({ x, y, width, height });
  };

  const onPointerUp = (e: PointerEvent) => {
    if (dragRef.current?.pointerId === e.pointerId) dragRef.current = null;
  };

  const pct = (v: number) => `${v * 100}%`;
  const handles: Handle[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];

  return (
    <div
      className="crop-overlay"
      style={{ left: box.left, top: box.top, width: box.width, height: box.height }}
    >
      <div
        className="crop-overlay__rect"
        style={{
          left: pct(crop.x),
          top: pct(crop.y),
          width: pct(crop.width),
          height: pct(crop.height),
        }}
        onPointerDown={beginDrag("move")}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
      >
        {/* Rule-of-thirds guides. */}
        <div className="crop-overlay__third crop-overlay__third--v" style={{ left: "33.333%" }} />
        <div className="crop-overlay__third crop-overlay__third--v" style={{ left: "66.667%" }} />
        <div className="crop-overlay__third crop-overlay__third--h" style={{ top: "33.333%" }} />
        <div className="crop-overlay__third crop-overlay__third--h" style={{ top: "66.667%" }} />
        {handles.map((h) => (
          <div
            key={h}
            className={`crop-overlay__handle crop-overlay__handle--${h}`}
            onPointerDown={beginDrag(h)}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={onPointerUp}
          />
        ))}
      </div>
    </div>
  );
}
