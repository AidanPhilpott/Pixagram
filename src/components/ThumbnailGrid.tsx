import type { MouseEvent } from "react";
import type { ImageEntry } from "../lib/ipc";

interface ThumbnailGridProps {
  entries: ImageEntry[];
  /** The photo whose preview is shown (always a member of `selectedIds`). */
  primaryId: string | null;
  /** Full multi-selection (Ctrl/Shift-click) — batch targets, e.g. presets. */
  selectedIds: ReadonlySet<string>;
  /** The click event rides along so the App can read Ctrl/Shift modifiers. */
  onSelect: (entry: ImageEntry, event: MouseEvent) => void;
}

/** The working-set grid. Thumbnails come from the camera JPEG for speed. */
export function ThumbnailGrid({
  entries,
  primaryId,
  selectedIds,
  onSelect,
}: ThumbnailGridProps) {
  if (entries.length === 0) {
    return (
      <div className="grid grid--empty">
        <p>No photos yet</p>
        <span>Import RAW files to build your working set</span>
      </div>
    );
  }

  const thumbClass = (id: string) => {
    if (id === primaryId) return "thumb thumb--selected";
    if (selectedIds.has(id)) return "thumb thumb--in-selection";
    return "thumb";
  };

  return (
    <div className="grid">
      {entries.map((entry) => (
        <button
          key={entry.id}
          className={thumbClass(entry.id)}
          onClick={(e) => onSelect(entry, e)}
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
