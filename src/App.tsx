import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent,
} from "react";
import { ThumbnailGrid } from "./components/ThumbnailGrid";
import { Preview } from "./components/Preview";
import { AdjustmentSlider } from "./components/AdjustmentSlider";
import { CollapsibleSection } from "./components/CollapsibleSection";
import { HslPanel } from "./components/HslPanel";
import { CurveEditor } from "./components/CurveEditor";
import { PresetPanel } from "./components/PresetPanel";
import {
  decodePreview,
  deletePreset,
  exportStub,
  listImages,
  listPresets,
  pickRafFiles,
  renamePreset,
  savePreset,
  type ImageEntry,
  type Preset,
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

/**
 * The neutral stack shown by the before/after toggle. A stable module constant
 * so `<Preview>` doesn't see a new object identity every render. Never mutated.
 */
const ORIGINAL_ADJUSTMENTS = defaultAdjustments();

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

/** Inspector slider rows, in display order. One row per scalar `Adjustments`
 *  field; `group` places it in a collapsible inspector section. */
const SLIDERS: ReadonlyArray<{
  field: ScalarField;
  label: string;
  group: "light" | "color";
  range: { min: number; max: number; step: number };
  format: (v: number) => string;
}> = [
  { field: "exposure", label: "Exposure", group: "light", range: EXPOSURE_RANGE, format: (v) => `${v >= 0 ? "+" : ""}${v.toFixed(2)} EV` },
  { field: "contrast", label: "Contrast", group: "light", range: CONTRAST_RANGE, format: asPercent },
  { field: "highlights", label: "Highlights", group: "light", range: TONE_RANGE, format: asPercent },
  { field: "shadows", label: "Shadows", group: "light", range: TONE_RANGE, format: asPercent },
  { field: "whites", label: "Whites", group: "light", range: TONE_RANGE, format: asPercent },
  { field: "blacks", label: "Blacks", group: "light", range: TONE_RANGE, format: asPercent },
  { field: "temperature", label: "Temperature", group: "color", range: WB_RANGE, format: asPercent },
  { field: "tint", label: "Tint", group: "color", range: WB_RANGE, format: asPercent },
  { field: "vibrance", label: "Vibrance", group: "color", range: COLOR_RANGE, format: asPercent },
  { field: "saturation", label: "Saturation", group: "color", range: COLOR_RANGE, format: asPercent },
];

/** Deep-copy a stack so an applied preset never aliases its arrays into a photo. */
function cloneAdjustments(a: Adjustments): Adjustments {
  return {
    ...a,
    hsl: a.hsl.map((b) => ({ ...b })),
    curve: a.curve.map((p) => ({ ...p })),
  };
}

export function App() {
  const [entries, setEntries] = useState<ImageEntry[]>([]);
  // Primary selection (drives the preview + inspector) plus the full
  // multi-selection (Ctrl/Shift-click) that batch operations target.
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
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

  // Before/after: momentary while "\" is held (Lightroom convention), or
  // latched via the inspector toggle. Never touches the stored stack — the
  // preview is just handed the neutral adjustments instead.
  const [holdOriginal, setHoldOriginal] = useState(false);
  const [pinOriginal, setPinOriginal] = useState(false);

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
  const showOriginal = (holdOriginal || pinOriginal) && !!selected;

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

  // Decode (or fetch from cache) the preview of the primary selection.
  const loadPreview = useCallback(async (entry: ImageEntry) => {
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

  // Grid click: plain = single select, Ctrl/Cmd = toggle membership, Shift =
  // range from the current primary. The primary drives the preview/inspector;
  // batch operations (preset apply) hit every selected id.
  const handleSelect = useCallback(
    async (entry: ImageEntry, e: MouseEvent) => {
      const additive = e.ctrlKey || e.metaKey;
      let next: Set<string>;
      let primary: string | null = entry.id;

      if (e.shiftKey && selectedId) {
        const ids = entries.map(({ id }) => id);
        const [a, b] = [ids.indexOf(selectedId), ids.indexOf(entry.id)].sort(
          (x, y) => x - y
        );
        const range = ids.slice(a, b + 1);
        next = additive ? new Set([...selectedIds, ...range]) : new Set(range);
      } else if (additive) {
        next = new Set(selectedIds);
        if (next.has(entry.id)) {
          // Toggled the clicked photo out; fall back to the last one still in.
          next.delete(entry.id);
          const rest = entries.filter(({ id }) => next.has(id));
          primary = rest.length > 0 ? rest[rest.length - 1].id : null;
        } else {
          next.add(entry.id);
        }
      } else {
        next = new Set([entry.id]);
      }

      setSelectedIds(next);
      setSelectedId(primary);
      setStatus(null);

      const target = entries.find(({ id }) => id === primary);
      if (!target) {
        setPreview(null);
        setPreviewLoading(false);
        return;
      }
      await loadPreview(target);
    },
    [entries, selectedId, selectedIds, loadPreview]
  );

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

  // Hold "\" for a momentary original view; release to return to the edit.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "\\" || e.repeat || isTextInput(e.target)) return;
      e.preventDefault();
      setHoldOriginal(true);
    };
    const onKeyUp = (e: KeyboardEvent) => {
      if (e.key === "\\") setHoldOriginal(false);
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, []);

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

  // One inspector row per SLIDERS entry (shared by the Light/Color sections).
  const sliderRow = (s: (typeof SLIDERS)[number]) => (
    <AdjustmentSlider
      key={s.field}
      id={s.field}
      label={s.label}
      value={adjustments[s.field]}
      min={s.range.min}
      max={s.range.max}
      step={s.range.step}
      defaultValue={0}
      disabled={!selected || showOriginal}
      onChange={(v) => updateField(s.field, v)}
      format={s.format}
    />
  );

  // ── Presets (Phase 3) — named `Adjustments` bundles from the Rust store ──
  const [presets, setPresets] = useState<Preset[]>([]);

  const refreshPresets = useCallback(async () => {
    try {
      setPresets(await listPresets());
    } catch (e) {
      setStatus(`Presets failed to load: ${e instanceof Error ? e.message : String(e)}`);
    }
  }, []);

  useEffect(() => {
    void refreshPresets();
  }, [refreshPresets]);

  // Replace the stack of every selected photo with the preset's adjustments.
  // Each photo gets its own undo snapshot, so an apply is one Ctrl+Z away —
  // batch writes can't go through applyEdit (it's single-photo), so this
  // pushes history itself.
  const applyPreset = useCallback(
    (preset: Preset) => {
      if (selectedIds.size === 0) return;
      const next = { ...stacksRef.current };
      for (const id of selectedIds) {
        const h = (historyRef.current[id] ??= { past: [], future: [] });
        h.past.push(next[id] ?? defaultAdjustments());
        if (h.past.length > HISTORY_CAP) h.past.shift();
        h.future = [];
        next[id] = cloneAdjustments(preset.adjustments);
      }
      lastEditRef.current = null; // the next edit starts a fresh undo step
      stacksRef.current = next;
      setStacks(next);
      const n = selectedIds.size;
      setStatus(`Applied "${preset.name}" to ${n} photo${n === 1 ? "" : "s"}`);
    },
    [selectedIds]
  );

  // Save/delete/rename report success back to the panel (so its inline name
  // inputs know whether to close) and surface errors in the status line.
  const handleSavePreset = useCallback(
    async (name: string) => {
      try {
        await savePreset(name, adjustments);
        await refreshPresets();
        setStatus(`Saved preset "${name}"`);
        return true;
      } catch (e) {
        setStatus(`Save preset failed: ${e instanceof Error ? e.message : String(e)}`);
        return false;
      }
    },
    [adjustments, refreshPresets]
  );

  const handleDeletePreset = useCallback(
    async (name: string) => {
      try {
        await deletePreset(name);
        await refreshPresets();
      } catch (e) {
        setStatus(`Delete preset failed: ${e instanceof Error ? e.message : String(e)}`);
      }
    },
    [refreshPresets]
  );

  const handleRenamePreset = useCallback(
    async (oldName: string, newName: string) => {
      try {
        await renamePreset(oldName, newName);
        await refreshPresets();
        return true;
      } catch (e) {
        setStatus(`Rename preset failed: ${e instanceof Error ? e.message : String(e)}`);
        return false;
      }
    },
    [refreshPresets]
  );

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
            primaryId={selectedId}
            selectedIds={selectedIds}
            onSelect={handleSelect}
          />
        </aside>

        <section className="stage">
          {showOriginal && <div className="stage__badge">Original</div>}
          <Preview
            preview={preview}
            adjustments={showOriginal ? ORIGINAL_ADJUSTMENTS : adjustments}
            loading={previewLoading}
          />
        </section>

        <aside className="inspector">
          <div className="inspector__head">
            <h2 className="inspector__title">
              {selected ? selected.fileName : "Adjustments"}
            </h2>
            <button
              type="button"
              className={`original-toggle${pinOriginal ? " original-toggle--on" : ""}`}
              onClick={() => setPinOriginal((v) => !v)}
              disabled={!selected}
              aria-pressed={pinOriginal}
              title={"Show the unedited original (hold \\ for a quick peek)"}
            >
              Original
            </button>
          </div>
          <CollapsibleSection id="light" title="Light" defaultOpen>
            {SLIDERS.filter((s) => s.group === "light").map(sliderRow)}
          </CollapsibleSection>
          <CollapsibleSection id="color" title="Color">
            {SLIDERS.filter((s) => s.group === "color").map(sliderRow)}
          </CollapsibleSection>
          <CollapsibleSection id="curve" title="Tone Curve">
            <CurveEditor
              points={adjustments.curve}
              disabled={!selected || showOriginal}
              onChange={updateCurve}
            />
          </CollapsibleSection>
          <CollapsibleSection id="hsl" title="HSL">
            <HslPanel
              bands={adjustments.hsl}
              disabled={!selected || showOriginal}
              onChange={updateHslBand}
            />
          </CollapsibleSection>
          <CollapsibleSection id="presets" title="Presets">
            <PresetPanel
              presets={presets}
              disabled={!selected || showOriginal}
              applyCount={selectedIds.size}
              onApply={applyPreset}
              onSaveCurrent={handleSavePreset}
              onDelete={handleDeletePreset}
              onRename={handleRenamePreset}
            />
          </CollapsibleSection>
          {status && <div className="inspector__status">{status}</div>}
        </aside>
      </main>
    </div>
  );
}
