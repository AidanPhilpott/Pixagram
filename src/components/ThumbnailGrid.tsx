import type { ImageEntry } from "../lib/ipc";

interface ThumbnailGridProps {
  entries: ImageEntry[];
  selectedId: string | null;
  onSelect: (entry: ImageEntry) => void;
}

/** The working-set grid. Thumbnails come from the camera JPEG for speed. */
export function ThumbnailGrid({ entries, selectedId, onSelect }: ThumbnailGridProps) {
  if (entries.length === 0) {
    return (
      <div className="grid grid--empty">
        <p>No photos yet</p>
        <span>Import RAFs to build your working set</span>
      </div>
    );
  }

  return (
    <div className="grid">
      {entries.map((entry) => (
        <button
          key={entry.id}
          className={`thumb${entry.id === selectedId ? " thumb--selected" : ""}`}
          onClick={() => onSelect(entry)}
          title={entry.fileName}
        >
          {entry.thumb ? (
            <img src={entry.thumb} alt={entry.fileName} loading="lazy" />
          ) : (
            <div className="thumb__fallback" title={entry.error ?? "No thumbnail"}>
              ⚠
            </div>
          )}
          <span className="thumb__label">{entry.fileName}</span>
        </button>
      ))}
    </div>
  );
}
