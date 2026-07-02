import { useEffect, useMemo, useRef, useState } from "react";
import { PreviewRenderer } from "../gl/PreviewRenderer";
import type { Preset } from "../lib/ipc";
import { fullFrameCrop, type Adjustments } from "../adjustments/definition";

/**
 * The bundled benchmark set (public/benchmark/, see the README there): the
 * classic Kodak test-suite images, picked for versatility — skin tones,
 * whites/sky/foliage, and saturated color — so a preset's character shows
 * at a glance.
 */
const BENCHMARKS = [
  { src: "/benchmark/portrait.jpg", label: "Portrait (skin tones)" },
  { src: "/benchmark/landscape.jpg", label: "Landscape (whites, sky)" },
  { src: "/benchmark/color.jpg", label: "Color (saturated primaries)" },
] as const;

/**
 * One benchmark image rendered through the REAL preview pipeline
 * (`PreviewRenderer` + the shared shader), so the example is exactly what the
 * preset does — not a pre-baked approximation. Selecting another preset is a
 * GPU-only uniform update, i.e. instant.
 */
function BenchmarkPreview({ src, adjustments }: { src: string; adjustments: Adjustments }) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const rendererRef = useRef<PreviewRenderer | null>(null);
  // The image may still be loading when the selection changes; render with
  // whatever adjustments are current once it lands.
  const adjRef = useRef(adjustments);
  adjRef.current = adjustments;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    let renderer: PreviewRenderer;
    try {
      renderer = new PreviewRenderer(canvas);
    } catch {
      return; // no WebGL — the modal just shows empty tiles
    }
    rendererRef.current = renderer;

    const img = new Image();
    img.onload = () => {
      if (rendererRef.current !== renderer) return; // unmounted meanwhile
      const scratch = document.createElement("canvas");
      scratch.width = img.naturalWidth;
      scratch.height = img.naturalHeight;
      const ctx = scratch.getContext("2d");
      if (!ctx) return;
      ctx.drawImage(img, 0, 0);
      const rgba = ctx.getImageData(0, 0, scratch.width, scratch.height).data;
      // 8-bit sRGB → the renderer's 16-bit layout (v * 257 spreads to 0..65535).
      const u16 = new Uint16Array(scratch.width * scratch.height * 3);
      for (let i = 0, j = 0; i < rgba.length; i += 4) {
        u16[j++] = rgba[i] * 257;
        u16[j++] = rgba[i + 1] * 257;
        u16[j++] = rgba[i + 2] * 257;
      }
      renderer.setImage(scratch.width, scratch.height, u16);
      renderer.setAdjustments(adjRef.current);
    };
    img.src = src;

    return () => {
      renderer.dispose();
      if (rendererRef.current === renderer) rendererRef.current = null;
    };
  }, [src]);

  useEffect(() => {
    rendererRef.current?.setAdjustments(adjustments);
  }, [adjustments]);

  return <canvas ref={canvasRef} className="preset-manager__bench" />;
}

interface PresetManagerModalProps {
  presets: Preset[];
  favorites: ReadonlySet<string>;
  onToggleFavorite: (name: string) => void;
  /** Apply the preset to the current selection (disabled when none). */
  onApply: (preset: Preset) => void;
  applyCount: number;
  canApply: boolean;
  onClose: () => void;
}

/**
 * The preset manager: browse EVERY preset (the inspector panel lists only
 * favorites), star/unstar favorites, and preview any preset live on the
 * benchmark images before applying it.
 */
export function PresetManagerModal({
  presets,
  favorites,
  onToggleFavorite,
  onApply,
  applyCount,
  canApply,
  onClose,
}: PresetManagerModalProps) {
  const [selectedName, setSelectedName] = useState<string | null>(
    presets.length > 0 ? presets[0].name : null
  );
  const selected = presets.find((p) => p.name === selectedName) ?? null;
  // Benchmarks preview the LOOK only — presets saved before geometry was
  // stripped at save time may still carry a rotation/crop; neutralize it.
  const previewAdjustments = useMemo(
    () =>
      selected
        ? { ...selected.adjustments, rotation: 0, straighten: 0, crop: fullFrameCrop() }
        : null,
    [selected]
  );

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className="modal-backdrop"
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="preset-manager" role="dialog" aria-label="Manage presets">
        <div className="preset-manager__head">
          <h2 className="preset-manager__title">Presets</h2>
          <span className="preset-manager__hint">
            ★ favorites appear in the inspector panel
          </span>
          <button className="preset-manager__close" onClick={onClose} title="Close (Esc)">
            ×
          </button>
        </div>
        <div className="preset-manager__body">
          <div className="preset-manager__list">
            {presets.map((p) => (
              <div
                key={p.name}
                className={`preset-manager__row${
                  p.name === selectedName ? " preset-manager__row--selected" : ""
                }`}
              >
                <button
                  className={`preset-manager__star${
                    favorites.has(p.name) ? " preset-manager__star--on" : ""
                  }`}
                  title={favorites.has(p.name) ? "Remove from favorites" : "Add to favorites"}
                  onClick={() => onToggleFavorite(p.name)}
                >
                  {favorites.has(p.name) ? "★" : "☆"}
                </button>
                <button
                  className="preset-manager__name"
                  onClick={() => setSelectedName(p.name)}
                  title={`Preview "${p.name}"`}
                >
                  {p.name}
                  {p.builtin && <span className="preset-row__tag">built-in</span>}
                </button>
              </div>
            ))}
          </div>
          <div className="preset-manager__preview">
            {selected && previewAdjustments ? (
              <>
                {BENCHMARKS.map((b) => (
                  <figure key={b.src} className="preset-manager__figure">
                    <BenchmarkPreview src={b.src} adjustments={previewAdjustments} />
                    <figcaption>{b.label}</figcaption>
                  </figure>
                ))}
                <button
                  className="btn btn--accent preset-manager__apply"
                  disabled={!canApply}
                  onClick={() => {
                    onApply(selected);
                    onClose();
                  }}
                >
                  {applyCount > 1
                    ? `Apply "${selected.name}" to ${applyCount} photos`
                    : `Apply "${selected.name}"`}
                </button>
              </>
            ) : (
              <p className="preset-manager__empty">Select a preset to preview it</p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
