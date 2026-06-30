import { useCallback, useMemo, useRef, useState } from "react";
import { ThumbnailGrid } from "./components/ThumbnailGrid";
import { Preview } from "./components/Preview";
import { AdjustmentSlider } from "./components/AdjustmentSlider";
import {
  decodePreview,
  exportStub,
  listImages,
  pickRafFiles,
  type ImageEntry,
  type PreviewPayload,
} from "./lib/ipc";
import {
  CONTRAST_RANGE,
  EXPOSURE_RANGE,
  TONE_RANGE,
  WB_RANGE,
  defaultAdjustments,
  type Adjustments,
} from "./adjustments/definition";

/** Compact readout shared by all unit-less [-1,1] sliders (shown as [-100,+100]). */
const asPercent = (v: number) => `${v > 0 ? "+" : ""}${Math.round(v * 100)}`;

/** Inspector slider rows, in display order. One row per `Adjustments` field. */
const SLIDERS: ReadonlyArray<{
  field: keyof Adjustments;
  label: string;
  range: { min: number; max: number; step: number };
  format: (v: number) => string;
}> = [
  { field: "exposure", label: "Exposure", range: EXPOSURE_RANGE, format: (v) => `${v >= 0 ? "+" : ""}${v.toFixed(2)} EV` },
  { field: "contrast", label: "Contrast", range: CONTRAST_RANGE, format: asPercent },
  { field: "temperature", label: "Temperature", range: WB_RANGE, format: asPercent },
  { field: "tint", label: "Tint", range: WB_RANGE, format: asPercent },
  { field: "highlights", label: "Highlights", range: TONE_RANGE, format: asPercent },
  { field: "shadows", label: "Shadows", range: TONE_RANGE, format: asPercent },
  { field: "whites", label: "Whites", range: TONE_RANGE, format: asPercent },
  { field: "blacks", label: "Blacks", range: TONE_RANGE, format: asPercent },
];

export function App() {
  const [entries, setEntries] = useState<ImageEntry[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [preview, setPreview] = useState<PreviewPayload | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  // Non-destructive adjustment stack per photo, keyed by id (= path).
  const [stacks, setStacks] = useState<Record<string, Adjustments>>({});

  const [importing, setImporting] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  // Cache developed previews by id so reselecting a photo doesn't re-decode the
  // RAF (brief: "cache decoded previews; don't re-decode on every interaction").
  const previewCache = useRef<Map<string, PreviewPayload>>(new Map());

  const selected = useMemo(
    () => entries.find((e) => e.id === selectedId) ?? null,
    [entries, selectedId]
  );
  const adjustments = selectedId
    ? stacks[selectedId] ?? defaultAdjustments()
    : defaultAdjustments();

  const handleImport = useCallback(async () => {
    try {
      setStatus(null);
      const paths = await pickRafFiles();
      if (paths.length === 0) return;
      setImporting(true);
      const imported = await listImages(paths);
      setEntries((prev) => {
        const byId = new Map(prev.map((e) => [e.id, e]));
        for (const e of imported) byId.set(e.id, e);
        return Array.from(byId.values());
      });
    } catch (e) {
      setStatus(`Import failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setImporting(false);
    }
  }, []);

  const handleSelect = useCallback(async (entry: ImageEntry) => {
    setSelectedId(entry.id);
    setStatus(null);

    const cached = previewCache.current.get(entry.id);
    if (cached) {
      setPreview(cached);
      setPreviewLoading(false);
      return;
    }

    setPreview(null);
    setPreviewLoading(true);
    try {
      const payload = await decodePreview(entry.path);
      previewCache.current.set(entry.id, payload);
      setPreview(payload);
    } catch (e) {
      setStatus(`Decode failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setPreviewLoading(false);
    }
  }, []);

  // Update a single field of the selected photo's stack. One handler scales to
  // every Phase 2 adjustment instead of a bespoke setter per slider.
  const updateField = useCallback(
    <K extends keyof Adjustments>(field: K, value: Adjustments[K]) => {
      if (!selectedId) return;
      setStacks((prev) => ({
        ...prev,
        [selectedId]: { ...(prev[selectedId] ?? defaultAdjustments()), [field]: value },
      }));
    },
    [selectedId]
  );

  const handleExport = useCallback(async () => {
    if (!selected) return;
    try {
      const msg = await exportStub(selected.path, adjustments);
      setStatus(msg);
    } catch (e) {
      setStatus(`Export failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  }, [selected, adjustments]);

  return (
    <div className="app">
      <header className="topbar">
        <div className="topbar__brand">
          <span className="topbar__mark" />
          Pixagram
        </div>
        <div className="topbar__actions">
          <button className="btn" onClick={handleImport} disabled={importing}>
            {importing ? "Importing…" : "Import RAF…"}
          </button>
          <button
            className="btn btn--accent"
            onClick={handleExport}
            disabled={!selected}
          >
            Export (stub)
          </button>
        </div>
      </header>

      <main className="workspace">
        <aside className="sidebar">
          <ThumbnailGrid
            entries={entries}
            selectedId={selectedId}
            onSelect={handleSelect}
          />
        </aside>

        <section className="stage">
          <Preview preview={preview} adjustments={adjustments} loading={previewLoading} />
        </section>

        <aside className="inspector">
          <h2 className="inspector__title">
            {selected ? selected.fileName : "Adjustments"}
          </h2>
          {SLIDERS.map((s) => (
            <AdjustmentSlider
              key={s.field}
              id={s.field}
              label={s.label}
              value={adjustments[s.field]}
              min={s.range.min}
              max={s.range.max}
              step={s.range.step}
              defaultValue={0}
              disabled={!selected}
              onChange={(v) => updateField(s.field, v)}
              format={s.format}
            />
          ))}
          {status && <div className="inspector__status">{status}</div>}
        </aside>
      </main>
    </div>
  );
}
