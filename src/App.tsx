import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ThumbnailGrid } from "./components/ThumbnailGrid";
import { Preview } from "./components/Preview";
import { AdjustmentSlider } from "./components/AdjustmentSlider";
import { HslPanel } from "./components/HslPanel";
import { CurveEditor } from "./components/CurveEditor";
import {
  decodePreview,
  exportStub,
  listImages,
  pickRafFiles,
  type ImageEntry,
  type PreviewPayload,
} from "./lib/ipc";
import {
  COLOR_RANGE,
  CONTRAST_RANGE,
  EXPOSURE_RANGE,
  TONE_RANGE,
  WB_RANGE,
  defaultAdjustments,
  type Adjustments,
  type CurvePoint,
  type HslBand,
} from "./adjustments/definition";

/** Compact readout shared by all unit-less [-1,1] sliders (shown as [-100,+100]). */
const asPercent = (v: number) => `${v > 0 ? "+" : ""}${Math.round(v * 100)}`;

/** Per-photo undo/redo snapshots (whole `Adjustments` values, newest last). */
type History = { past: Adjustments[]; future: Adjustments[] };
const HISTORY_CAP = 100;
/**
 * Edits with the same key landing within this window merge into the previous
 * undo step — a slider/curve/HSL drag is a stream of change events, and must
 * undo as ONE step, not hundreds.
 */
const COALESCE_MS = 400;

/** True when the keyboard focus is somewhere text is being typed. */
const isTextInput = (t: EventTarget | null) => {
  const el = t as HTMLElement | null;
  if (!el) return false;
  if (el.isContentEditable || el.tagName === "TEXTAREA") return true;
  return (
    el.tagName === "INPUT" &&
    !["range", "checkbox", "radio", "button", "submit"].includes(
      (el as HTMLInputElement).type
    )
  );
};

/** The scalar (single-number) fields of `Adjustments` — i.e. everything but `hsl`. */
type ScalarField = {
  [K in keyof Adjustments]: Adjustments[K] extends number ? K : never;
}[keyof Adjustments];

/** Inspector slider rows, in display order. One row per scalar `Adjustments` field. */
const SLIDERS: ReadonlyArray<{
  field: ScalarField;
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
  { field: "vibrance", label: "Vibrance", range: COLOR_RANGE, format: asPercent },
  { field: "saturation", label: "Saturation", range: COLOR_RANGE, format: asPercent },
];

export function App() {
  const [entries, setEntries] = useState<ImageEntry[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [preview, setPreview] = useState<PreviewPayload | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  // Non-destructive adjustment stack per photo, keyed by id (= path).
  const [stacks, setStacks] = useState<Record<string, Adjustments>>({});
  // Synchronous mirror of `stacks` so event handlers can read the pre-edit value
  // without waiting for a re-render (all writes go through applyEdit/timeTravel).
  const stacksRef = useRef(stacks);
  // Undo/redo history per photo. Refs, not state — history changes never need a
  // re-render of their own, and mutating refs in handlers is StrictMode-safe.
  const historyRef = useRef<Record<string, History>>({});
  // Identity of the last edit, for drag coalescing (see applyEdit).
  const lastEditRef = useRef<{ id: string; key: string; time: number } | null>(null);

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

  // Single choke point for every stack write. Pushes an undo snapshot unless
  // the edit continues the previous one (same photo + same key, within
  // COALESCE_MS) — so a whole drag collapses into one undo step while live
  // state still updates per event. Any future stack replacement (preset apply,
  // reset-all) routed through here is a single undoable step for free.
  const applyEdit = useCallback(
    (key: string, edit: (current: Adjustments) => Adjustments) => {
      if (!selectedId) return;
      const current = stacksRef.current[selectedId] ?? defaultAdjustments();
      const now = Date.now();
      const last = lastEditRef.current;
      const coalesce =
        last && last.id === selectedId && last.key === key && now - last.time < COALESCE_MS;
      if (!coalesce) {
        const h = (historyRef.current[selectedId] ??= { past: [], future: [] });
        h.past.push(current);
        if (h.past.length > HISTORY_CAP) h.past.shift();
        h.future = []; // a fresh edit invalidates the redo branch
      }
      lastEditRef.current = { id: selectedId, key, time: now };
      const next = { ...stacksRef.current, [selectedId]: edit(current) };
      stacksRef.current = next;
      setStacks(next);
    },
    [selectedId]
  );

  // Step the selected photo's stack backward/forward through its history.
  const timeTravel = useCallback(
    (direction: "undo" | "redo") => {
      if (!selectedId) return;
      const h = historyRef.current[selectedId];
      const source = direction === "undo" ? h?.past : h?.future;
      if (!h || !source || source.length === 0) return;
      const current = stacksRef.current[selectedId] ?? defaultAdjustments();
      const target = source.pop()!;
      (direction === "undo" ? h.future : h.past).push(current);
      lastEditRef.current = null; // the next edit starts a fresh undo step
      const next = { ...stacksRef.current, [selectedId]: target };
      stacksRef.current = next;
      setStacks(next);
    },
    [selectedId]
  );

  // Global undo/redo shortcuts: Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z (Cmd on macOS).
  // Skipped while typing so the browser's native text undo is never stolen.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
      const k = e.key.toLowerCase();
      if (k !== "z" && k !== "y") return;
      if (isTextInput(e.target)) return;
      e.preventDefault();
      timeTravel(k === "y" || e.shiftKey ? "redo" : "undo");
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [timeTravel]);

  // Update a single field of the selected photo's stack. One handler scales to
  // every Phase 2 adjustment instead of a bespoke setter per slider.
  const updateField = useCallback(
    <K extends keyof Adjustments>(field: K, value: Adjustments[K]) => {
      applyEdit(String(field), (cur) => ({ ...cur, [field]: value }));
    },
    [applyEdit]
  );

  // Update one field of one HSL band, leaving the other 7 bands untouched.
  const updateHslBand = useCallback(
    (index: number, field: keyof HslBand, value: number) => {
      applyEdit(`hsl.${index}.${field}`, (cur) => ({
        ...cur,
        hsl: cur.hsl.map((b, i) => (i === index ? { ...b, [field]: value } : b)),
      }));
    },
    [applyEdit]
  );

  // Replace the whole tone curve (the editor manages add/move/remove).
  const updateCurve = useCallback(
    (curve: CurvePoint[]) => {
      applyEdit("curve", (cur) => ({ ...cur, curve }));
    },
    [applyEdit]
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
          <CurveEditor points={adjustments.curve} disabled={!selected} onChange={updateCurve} />
          <HslPanel bands={adjustments.hsl} disabled={!selected} onChange={updateHslBand} />
          {status && <div className="inspector__status">{status}</div>}
        </aside>
      </main>
    </div>
  );
}
