/** Typed wrappers around the Tauri IPC commands exposed by the Rust core. */
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Adjustments } from "../adjustments/definition";

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
  dataUrl: string;
}

/** Open a native file picker for one or many Fujifilm RAF files. */
export async function pickRafFiles(): Promise<string[]> {
  const selection = await open({
    multiple: true,
    directory: false,
    filters: [{ name: "Fujifilm RAW", extensions: ["raf", "RAF"] }],
  });
  if (selection == null) return [];
  return Array.isArray(selection) ? selection : [selection];
}

/** Build thumbnail-grid entries from selected RAF paths. */
export function listImages(paths: string[]): Promise<ImageEntry[]> {
  return invoke<ImageEntry[]>("list_images", { paths });
}

/** Decode + develop one RAF into a downscaled sRGB preview. */
export function decodePreview(path: string): Promise<PreviewPayload> {
  return invoke<PreviewPayload>("decode_preview", { path });
}

/** Run the Phase 1 export stub for one photo with its adjustment stack. */
export function exportStub(path: string, adjustments: Adjustments): Promise<string> {
  return invoke<string>("export_stub", { path, adjustments });
}
