import { useState, type KeyboardEvent } from "react";
import type { Preset } from "../lib/ipc";

interface PresetPanelProps {
  presets: Preset[];
  /** Names of favorited presets — the ONLY ones this panel lists. */
  favorites: ReadonlySet<string>;
  /** The preset the selected photo's stack currently equals (look-wise), or
   *  null once any edit has dirtied it into a custom setting. */
  activePreset: string | null;
  /** No photo selected — applying and saving make no sense. */
  disabled: boolean;
  /** How many photos an apply currently targets (grid multi-select). */
  applyCount: number;
  onApply: (preset: Preset) => void;
  /** Async handlers resolve `true` on success so the inline inputs can close. */
  onSaveCurrent: (name: string) => Promise<boolean>;
  onDelete: (name: string) => void;
  onRename: (oldName: string, newName: string) => Promise<boolean>;
  /** Open the preset manager (all presets + favorites + benchmark previews). */
  onOpenManager: () => void;
}

/**
 * The quick-access preset shelf: shows only FAVORITES (curated in the preset
 * manager — see `PresetManagerModal`), so the panel stays short as the library
 * grows. Click a preset to apply it to every selected photo. A preset is just
 * a named `Adjustments` bundle (model A), so saving/renaming is inline naming.
 */
export function PresetPanel({
  presets,
  favorites,
  activePreset,
  disabled,
  applyCount,
  onApply,
  onSaveCurrent,
  onDelete,
  onRename,
  onOpenManager,
}: PresetPanelProps) {
  const [saveOpen, setSaveOpen] = useState(false);
  const [saveName, setSaveName] = useState("");
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");

  const submitSave = async () => {
    const name = saveName.trim();
    if (!name) return;
    if (await onSaveCurrent(name)) {
      setSaveOpen(false);
      setSaveName("");
    }
  };

  const submitRename = async () => {
    const name = renameDraft.trim();
    if (!renaming || !name) return;
    if (name === renaming || (await onRename(renaming, name))) setRenaming(null);
  };

  /** Enter submits, Escape cancels — shared by both inline name inputs. */
  const nameKeys =
    (submit: () => void, cancel: () => void) => (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") submit();
      if (e.key === "Escape") cancel();
    };

  return (
    <div className="presets">
      {/* The collapsible section header carries the title; this row is the
          batch hint only. */}
      {applyCount > 1 && (
        <div className="presets__head">
          <span className="presets__count">applies to {applyCount} photos</span>
        </div>
      )}

      <div className="presets__list">
        {presets.filter((p) => favorites.has(p.name)).length === 0 && (
          <p className="presets__empty">
            No favorite presets — star some in Manage presets below.
          </p>
        )}
        {presets.filter((p) => favorites.has(p.name)).map((p) =>
          renaming === p.name ? (
            <div className="preset-row" key={p.name}>
              <input
                className="presets__input"
                value={renameDraft}
                autoFocus
                onChange={(e) => setRenameDraft(e.target.value)}
                onKeyDown={nameKeys(submitRename, () => setRenaming(null))}
              />
              <button className="preset-row__action" onClick={submitRename}>
                OK
              </button>
              <button className="preset-row__action" onClick={() => setRenaming(null)}>
                Cancel
              </button>
            </div>
          ) : (
            <div
              className={`preset-row${
                p.name === activePreset ? " preset-row--active" : ""
              }`}
              key={p.name}
            >
              <button
                className="preset-row__apply"
                disabled={disabled}
                title={`Apply "${p.name}"`}
                onClick={() => onApply(p)}
              >
                <span className="preset-row__name">
                  {p.name === activePreset && (
                    <span className="preset-row__check" aria-label="Active">
                      ✓{" "}
                    </span>
                  )}
                  {p.name}
                </span>
                {p.builtin && <span className="preset-row__tag">built-in</span>}
              </button>
              {!p.builtin && (
                <>
                  <button
                    className="preset-row__action"
                    title="Rename preset"
                    onClick={() => {
                      setRenaming(p.name);
                      setRenameDraft(p.name);
                    }}
                  >
                    Rename
                  </button>
                  <button
                    className="preset-row__action preset-row__action--danger"
                    title="Delete preset"
                    onClick={() => onDelete(p.name)}
                  >
                    Delete
                  </button>
                </>
              )}
            </div>
          )
        )}
      </div>

      {saveOpen ? (
        <div className="preset-row">
          <input
            className="presets__input"
            placeholder="Preset name"
            value={saveName}
            autoFocus
            onChange={(e) => setSaveName(e.target.value)}
            onKeyDown={nameKeys(submitSave, () => setSaveOpen(false))}
          />
          <button className="preset-row__action" onClick={submitSave}>
            Save
          </button>
          <button className="preset-row__action" onClick={() => setSaveOpen(false)}>
            Cancel
          </button>
        </div>
      ) : (
        <button
          className="btn presets__save-btn"
          disabled={disabled}
          onClick={() => setSaveOpen(true)}
        >
          Save current as preset…
        </button>
      )}
      <button className="btn presets__save-btn presets__manage-btn" onClick={onOpenManager}>
        Manage presets…
      </button>
    </div>
  );
}
