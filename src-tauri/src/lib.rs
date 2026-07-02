//! Pixagram core — thin Tauri shell over the portable image engine.
//!
//! IPC surface:
//! - `list_images`   — build grid entries (path + camera-JPEG thumbnail).
//! - `decode_preview` — lazily decode/develop one RAF into a downscaled preview.
//! - `export_stub`   — confirm the shared `Adjustments` reaches Rust end-to-end.
//! - `save_stacks` / `load_stacks` — persist the per-image edit stacks as JSON.
//!
//! Phase 3 (presets):
//! - `list_presets` / `save_preset` / `delete_preset` / `rename_preset` — the
//!   JSON preset store under `<app-data>/presets/` (see `presets.rs`).

mod adjustments;
mod develop;
mod export;
mod presets;
mod raf;
mod store;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use adjustments::Adjustments;
use tauri::Manager;

/// Diagnostic hook: run the owned develop pipeline directly (used by the
/// `develop_check` example to validate color against the camera JPEG).
#[doc(hidden)]
pub fn develop_for_test(
    path: &Path,
    max_edge: u32,
) -> anyhow::Result<image::DynamicImage> {
    develop::develop_raw(path, max_edge)
}

/// One entry in the working-set thumbnail grid.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageEntry {
    /// Stable id for the frontend — the absolute path doubles as the key.
    pub id: String,
    pub path: String,
    pub file_name: String,
    /// JPEG `data:` URL thumbnail, or `null` if it could not be generated.
    pub thumb: Option<String>,
    /// Non-fatal per-file error message, if thumbnailing failed.
    pub error: Option<String>,
}

/// Build grid entries for a set of selected RAF paths. Per-file failures are
/// reported in the entry rather than failing the whole import.
#[tauri::command]
fn list_images(paths: Vec<String>) -> Vec<ImageEntry> {
    paths
        .into_iter()
        .map(|path| {
            let file_name = Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());

            match raf::thumbnail(&path) {
                Ok(thumb) => ImageEntry {
                    id: path.clone(),
                    path,
                    file_name,
                    thumb: Some(thumb),
                    error: None,
                },
                Err(e) => ImageEntry {
                    id: path.clone(),
                    path,
                    file_name,
                    thumb: None,
                    error: Some(e.to_string()),
                },
            }
        })
        .collect()
}

/// Decode + develop a single RAF into a downscaled sRGB preview.
#[tauri::command]
fn decode_preview(path: String) -> Result<raf::PreviewPayload, String> {
    raf::preview(&path).map_err(|e| e.to_string())
}

/// Phase 1 export stub — proves the shared adjustment definition reaches Rust.
#[tauri::command]
fn export_stub(path: String, adjustments: Adjustments) -> Result<String, String> {
    export::export_stub(&path, &adjustments).map_err(|e| e.to_string())
}

/// Reveal a file in the OS file manager (Explorer on Windows, Finder on
/// macOS, the containing directory elsewhere). Backs the thumbnail context
/// menu's "Show in Explorer".
#[tauri::command]
fn reveal_in_file_manager(path: String) -> Result<(), String> {
    if !Path::new(&path).exists() {
        return Err(format!("file no longer exists: {path}"));
    }
    #[cfg(target_os = "windows")]
    {
        // `/select,<path>` must be ONE argument — Explorer's parser splits on
        // the comma itself and quotes around the combined arg are fine.
        std::process::Command::new("explorer")
            .arg(format!("/select,{path}"))
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let dir = Path::new(&path)
            .parent()
            .ok_or_else(|| "no parent directory".to_string())?;
        std::process::Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Resolve the on-disk preset store (`<app-data>/presets/`). The store
/// functions take the directory explicitly so tests can inject a temp dir.
fn presets_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("presets"))
        .map_err(|e| e.to_string())
}

/// List built-in + user presets (built-ins first).
#[tauri::command]
fn list_presets(app: tauri::AppHandle) -> Result<Vec<presets::Preset>, String> {
    presets::list_presets(&presets_dir(&app)?).map_err(|e| e.to_string())
}

/// Save (or overwrite) a user preset holding one `Adjustments` stack.
#[tauri::command]
fn save_preset(
    app: tauri::AppHandle,
    name: String,
    adjustments: Adjustments,
) -> Result<(), String> {
    presets::save_preset(&presets_dir(&app)?, &name, &adjustments).map_err(|e| e.to_string())
}

/// Delete a user preset by name (built-ins are refused).
#[tauri::command]
fn delete_preset(app: tauri::AppHandle, name: String) -> Result<(), String> {
    presets::delete_preset(&presets_dir(&app)?, &name).map_err(|e| e.to_string())
}

/// Rename a user preset (built-ins are refused; the target name must be free).
#[tauri::command]
fn rename_preset(
    app: tauri::AppHandle,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    presets::rename_preset(&presets_dir(&app)?, &old_name, &new_name).map_err(|e| e.to_string())
}

/// Where the edit-stack store lives: `<app data dir>/edit-stacks.json`. The
/// store functions themselves take the path (keeps them unit-testable).
fn stacks_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("edit-stacks.json"))
        .map_err(|e| e.to_string())
}

/// Persist the full per-image edit-stack map (last-write-wins).
#[tauri::command]
fn save_stacks(app: tauri::AppHandle, stacks: HashMap<String, Adjustments>) -> Result<(), String> {
    store::save_stacks_to(&stacks_path(&app)?, stacks).map_err(|e| e.to_string())
}

/// Load the persisted per-image edit stacks (empty map on first run).
#[tauri::command]
fn load_stacks(app: tauri::AppHandle) -> Result<HashMap<String, Adjustments>, String> {
    store::load_stacks_from(&stacks_path(&app)?).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_images,
            decode_preview,
            export_stub,
            reveal_in_file_manager,
            list_presets,
            save_preset,
            delete_preset,
            rename_preset,
            save_stacks,
            load_stacks
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pixagram");
}
