import { useRef, useState } from "react";
import { evalCurve } from "../adjustments/curve";
import { identityCurve, type CurvePoint } from "../adjustments/definition";

interface CurveEditorProps {
  points: CurvePoint[];
  disabled: boolean;
  onChange: (points: CurvePoint[]) => void;
}

const MAX_POINTS = 16;
const SAMPLES = 48; // path resolution
const clamp01 = (v: number) => (v < 0 ? 0 : v > 1 ? 1 : v);
const isIdentity = (p: CurvePoint[]) =>
  p.length === 2 && p[0].x === 0 && p[0].y === 0 && p[1].x === 1 && p[1].y === 1;

/** Data point (x,y in [0,1]) → SVG coords in a 0..100 box (y flipped). */
const sx = (x: number) => x * 100;
const sy = (y: number) => (1 - y) * 100;

/**
 * Interactive tone curve. Drag points to reshape; click an empty spot to add a
 * point; double-click a point to remove it. Endpoints lock their x. The drawn
 * spline matches the export exactly (shared `evalCurve`).
 */
export function CurveEditor({ points, disabled, onChange }: CurveEditorProps) {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [drag, setDrag] = useState<number | null>(null);
  // Manual double-tap tracking: pointer capture (needed for dragging) swallows the
  // native dblclick, so we detect a quick second tap on the same point ourselves.
  const lastTap = useRef<{ index: number; time: number } | null>(null);

  const toData = (e: { clientX: number; clientY: number }): CurvePoint => {
    const rect = svgRef.current!.getBoundingClientRect();
    return {
      x: clamp01((e.clientX - rect.left) / rect.width),
      y: clamp01(1 - (e.clientY - rect.top) / rect.height),
    };
  };

  const startDragPoint = (index: number) => (e: React.PointerEvent) => {
    if (disabled) return;
    e.stopPropagation();
    const now = Date.now();
    const last = lastTap.current;
    const isInterior = index !== 0 && index !== points.length - 1;
    if (last && last.index === index && now - last.time < 350 && isInterior) {
      // Second quick tap on an interior point → remove it.
      lastTap.current = null;
      onChange(points.filter((_, i) => i !== index));
      return;
    }
    lastTap.current = { index, time: now };
    setDrag(index);
    svgRef.current?.setPointerCapture(e.pointerId);
  };

  const addPoint = (e: React.PointerEvent) => {
    if (disabled || points.length >= MAX_POINTS) return;
    const d = toData(e);
    if (d.x <= 0 || d.x >= 1) return; // can't add on the locked endpoints
    const index = points.filter((p) => p.x < d.x).length;
    const next = [...points.slice(0, index), d, ...points.slice(index)];
    onChange(next);
    setDrag(index);
    svgRef.current?.setPointerCapture(e.pointerId);
  };

  const move = (e: React.PointerEvent) => {
    if (disabled || drag === null) return;
    const d = toData(e);
    const next = points.slice();
    const isEndpoint = drag === 0 || drag === next.length - 1;
    let x = d.x;
    if (isEndpoint) {
      x = next[drag].x; // endpoints keep their x (0 or 1)
    } else {
      const lo = next[drag - 1].x + 0.001;
      const hi = next[drag + 1].x - 0.001;
      x = Math.min(Math.max(d.x, lo), hi);
    }
    next[drag] = { x, y: d.y };
    onChange(next);
  };

  const endDrag = (e: React.PointerEvent) => {
    if (drag === null) return;
    setDrag(null);
    svgRef.current?.releasePointerCapture(e.pointerId);
  };

  // Build the spline path from the shared evaluator.
  let path = `M ${sx(0)} ${sy(evalCurve(points, 0))}`;
  for (let i = 1; i <= SAMPLES; i++) {
    const x = i / SAMPLES;
    path += ` L ${sx(x).toFixed(2)} ${sy(evalCurve(points, x)).toFixed(2)}`;
  }

  return (
    <div className={`curve${disabled ? " curve--disabled" : ""}`}>
      <div className="curve__head">
        <label>Tone Curve</label>
        {!isIdentity(points) && (
          <button
            className="control__reset"
            onClick={() => onChange(identityCurve())}
            disabled={disabled}
            title="Reset tone curve"
          >
            Reset
          </button>
        )}
      </div>
      <svg
        ref={svgRef}
        className="curve__canvas"
        viewBox="0 0 100 100"
        preserveAspectRatio="none"
        onPointerMove={move}
        onPointerUp={endDrag}
        onPointerLeave={endDrag}
      >
        {/* grid */}
        <rect className="curve__bg" x="0" y="0" width="100" height="100" onPointerDown={addPoint} />
        {[25, 50, 75].map((p) => (
          <g key={p} className="curve__grid">
            <line x1={p} y1="0" x2={p} y2="100" />
            <line x1="0" y1={p} x2="100" y2={p} />
          </g>
        ))}
        <line className="curve__identity" x1="0" y1="100" x2="100" y2="0" />
        <path className="curve__line" d={path} />
        {points.map((p, i) => (
          <circle
            key={i}
            className="curve__point"
            cx={sx(p.x)}
            cy={sy(p.y)}
            r="2.4"
            onPointerDown={startDragPoint(i)}
            onPointerCancel={endDrag}
          />
        ))}
      </svg>
      <div className="curve__hint">Click to add · drag to shape · double-click to remove</div>
    </div>
  );
}
