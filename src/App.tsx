import { useCallback, useMemo, useState } from "react";
import { ThumbnailGrid } from "./components/ThumbnailGrid";
import { Preview } from "./components/Preview";
import { ExposureSlider } from "./components/ExposureSlider";
import {
  decodePreview,
  exportStub,
  listImages,
  pickRafFiles,
  type ImageEntry,
  type PreviewPayload,
} from "./lib/ipc";
import { defaultAdjustments, type Adjustments } from "./adjustments/definition";

export function App() {
  const [entries, setEntries] = useState<ImageEntry[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [preview, setPreview] = useState<PreviewPayload | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  // Non-destructive adjustment stack per photo, keyed by id (= path).
  const [stacks, setStacks] = useState<Record<string, Adjustments>>({});

  const [importing, setImporting] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

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
    setPreview(null);
    setPreviewLoading(true);
    setStatus(null);
    try {
      const payload = await decodePreview(entry.path);
      setPreview(payload);
    } catch (e) {
      setStatus(`Decode failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setPreviewLoading(false);
    }
  }, []);

  const handleExposure = useCallback(
    (ev: number) => {
      if (!selectedId) return;
      setStacks((prev) => ({
        ...prev,
        [selectedId]: { ...(prev[selectedId] ?? defaultAdjustments()), exposure: ev },
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
          <Preview preview={preview} exposure={adjustments.exposure} loading={previewLoading} />
        </section>

        <aside className="inspector">
          <h2 className="inspector__title">
            {selected ? selected.fileName : "Adjustments"}
          </h2>
          <ExposureSlider
            value={adjustments.exposure}
            disabled={!selected}
            onChange={handleExposure}
          />
          {status && <div className="inspector__status">{status}</div>}
        </aside>
      </main>
    </div>
  );
}
