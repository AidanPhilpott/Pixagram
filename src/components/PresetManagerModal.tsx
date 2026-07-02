import { useEffect, useMemo, useRef, useState } from "react";
import { PreviewRenderer } from "../gl/PreviewRenderer";
import { decodeU16, rgbaToU16 } from "../lib/pixels";
import type { Preset, PreviewPayload } from "../lib/ipc";
import { fullFrameCrop, type Adjustments } from "../adjustments/definition";

/**
 * The bundled benchmark set (public/benchmark/, see the README there): the
 * classic Kodak test-suite images plus two CC0 landscapes, picked for
 * versatility — skin tones, whites/sky, saturated color, lush greens, and
 * snow — so a preset's character shows at a glance.
 */
const BENCHMARKS = [
  { src: "/benchmark/portrait.jpg", label: "Portrait" },
  { src: "/benchmark/landscape.jpg", label: "Lighthouse" },
  { src: "/benchmark/color.jpg", label: "Saturated color" },
  { src: "/benchmark/rainforest.jpg", label: "Rainforest greens" },
  { src: "/benchmark/snowy.jpg", label: "Snowy range" },
] as const;

/** What a preview tile renders: a bundled benchmark URL, or raw preview pixels. */
type TileImage = { src: string } | { payload: PreviewPayload };

/**
 * One preview tile rendered through the REAL preview pipeline
 * (`PreviewRenderer` + the shared shader), so the example is exactly what the
 * preset does — not a pre-baked approximation. Selecting another preset is a
 * GPU-only uniform update, i.e. instant.
 */
function PresetTile({ image, adjustments }: { image: TileImage; adjustments: Adjustments }) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const rendererRef = useRef<PreviewRenderer | null>(null);
  // The image may still be loading when the selection changes; render with
  // whatever adjustments are current once it lands.
  const adjRef = useRef(adjustments);
  adjRef.current = adjustments;

  const src = "src" in image ? image.src : null;
  const payload = "payload" in image ? image.payload : null;

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

    if (payload) {
      // The current photo's developed preview — already the renderer's format.
      try {
        renderer.setImage(payload.width, payload.height, decodeU16(payload.data));
        renderer.setAdjustments(adjRef.current);
      } catch {
        /* tile stays empty */
      }
    } else if (src) {
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
        renderer.setImage(scratch.width, scratch.height, rgbaToU16(rgba));
        renderer.setAdjustments(adjRef.current);
      };
      img.src = src;
    }

    return () => {
      renderer.dispose();
      if (rendererRef.current === renderer) rendererRef.current = null;
    };
  }, [src, payload]);

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
  /** The selected photo's developed preview, for the "Your photo" source. */
  currentPreview: PreviewPayload | null;
  /** The selected photo's file name (labels the "Your photo" tile). */
  currentLabel: string | null;
  onClose: () => void;
}

/**
 * The preset manager: browse EVERY preset (the inspector panel lists only
 * favorites), star/unstar favorites, and preview any preset live — on the
 * bundled benchmark tiles, or on the photo currently selected in the grid.
 */
export function PresetManagerModal({
  presets,
  favorites,
  onToggleFavorite,
  onApply,
  applyCount,
  canApply,
  currentPreview,
  currentLabel,
  onClose,
}: PresetManagerModalProps) {
  const [selectedName, setSelectedName] = useState<string | null>(
    presets.length > 0 ? presets[0].name : null
  );
  // Preview source: the benchmark examples, or the currently selected photo.
  const [source, setSource] = useState<"examples" | "photo">("examples");
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

  const showPhoto = source === "photo" && currentPreview;

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
          <div className="preset-manager__source" role="tablist" aria-label="Preview source">
            <button
              className={`preset-manager__source-btn${
                source === "examples" ? " preset-manager__source-btn--on" : ""
              }`}
              onClick={() => setSource("examples")}
            >
              Examples
            </button>
            <button
              className={`preset-manager__source-btn${
                source === "photo" ? " preset-manager__source-btn--on" : ""
              }`}
              disabled={!currentPreview}
              title={currentPreview ? undefined : "Select a photo first"}
              onClick={() => setSource("photo")}
            >
              Your photo
            </button>
          </div>
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
                <div
                  className={`preset-manager__tiles${
                    showPhoto ? " preset-manager__tiles--single" : ""
                  }`}
                >
                  {showPhoto ? (
                    <figure className="preset-manager__figure" key="current-photo">
                      <PresetTile
                        image={{ payload: currentPreview }}
                        adjustments={previewAdjustments}
                      />
                      <figcaption>{currentLabel ?? "Current photo"}</figcaption>
                    </figure>
                  ) : (
                    BENCHMARKS.map((b) => (
                      <figure key={b.src} className="preset-manager__figure">
                        <PresetTile image={{ src: b.src }} adjustments={previewAdjustments} />
                        <figcaption>{b.label}</figcaption>
                      </figure>
                    ))
                  )}
                </div>
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
