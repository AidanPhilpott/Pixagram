/** Typed wrappers around the Tauri IPC commands exposed by the Rust core. */
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Adjustments } from "../adjustments/definition";
import type { Preset } from "../adjustments/generated/Preset";

/** ts-rs mirror of the Rust `Preset` struct (a named `Adjustments` bundle). */
export type { Preset };

/** Mirror of the Rust `ImageEntry` struct. */
export interface ImageEntry {
  id: string;
  path: string;
  fileName: string;
  thumb: string | null;
  error: string | null;
}

/** Mirror of the Rust `PreviewPayload` struct. */
export interface PreviewPayload {
  width: number;
  height: number;
  /** Base64 of raw RGB pixels as little-endian u16 (sRGB-encoded, row 0 at top). */
  data: string;
}

/** Raw formats the develop pipeline supports (rawler-decoded, Bayer or X-Trans). */
const RAW_EXTENSIONS = ["raf", "nef", "cr2", "cr3", "arw", "dng", "orf", "rw2", "pef"];

/** Open a native file picker for one or many camera raw files. */
export async function pickRawFiles(): Promise<string[]> {
  const selection = await open({
    multiple: true,
    directory: false,
    filters: [
      {
        name: "Camera RAW",
        // Both cases: some platforms' pickers match extensions case-sensitively.
        extensions: [...RAW_EXTENSIONS, ...RAW_EXTENSIONS.map((e) => e.toUpperCase())],
      },
    ],
  });
  if (selection == null) return [];
  return Array.isArray(selection) ? selection : [selection];
}

/** Build thumbnail-grid entries from selected raw paths. */
export function listImages(paths: string[]): Promise<ImageEntry[]> {
  return invoke<ImageEntry[]>("list_images", { paths });
}

/** Decode + develop one raw file into a downscaled sRGB preview. */
export function decodePreview(path: string): Promise<PreviewPayload> {
  return invoke<PreviewPayload>("decode_preview", { path });
}

/** Run the Phase 1 export stub for one photo with its adjustment stack. */
export function exportStub(path: string, adjustments: Adjustments): Promise<string> {
  return invoke<string>("export_stub", { path, adjustments });
}

/** List built-in + user presets (built-ins first). */
export function listPresets(): Promise<Preset[]> {
  return invoke<Preset[]>("list_presets");
}

/** Save (or overwrite) a user preset holding the given adjustment stack. */
export function savePreset(name: string, adjustments: Adjustments): Promise<void> {
  return invoke("save_preset", { name, adjustments });
}

/** Delete a user preset by name (built-ins are refused by the Rust side). */
export function deletePreset(name: string): Promise<void> {
  return invoke("delete_preset", { name });
}

/** Rename a user preset (built-ins are refused by the Rust side). */
export function renamePreset(oldName: string, newName: string): Promise<void> {
  return invoke("rename_preset", { oldName, newName });
}

/** Load the persisted per-image edit stacks (empty map on first run). */
export function loadStacks(): Promise<Record<string, Adjustments>> {
  return invoke<Record<string, Adjustments>>("load_stacks");
}

/** Persist the full per-image edit-stack map (last-write-wins). */
export function saveStacks(stacks: Record<string, Adjustments>): Promise<void> {
  return invoke("save_stacks", { stacks });
}
