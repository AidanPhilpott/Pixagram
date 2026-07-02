//! Per-image edit-stack persistence.
//!
//! One pretty-printed JSON file (`edit-stacks.json` in the app data dir) holds
//! the whole map of image path → `Adjustments`. Lightly versioned; every
//! `Adjustments` field is `#[serde(default)]`, so the file keeps loading as the
//! struct grows. Last-write-wins, no locking — the frontend debounces saves and
//! there is only ever one writer (the app itself).
//!
//! The path is injected by the caller (`lib.rs` resolves the app data dir) so
//! the round-trip is unit-testable against a temp file.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::adjustments::Adjustments;

/// Bump only on a breaking layout change; additive `Adjustments` growth is
/// absorbed by serde defaults and needs no bump.
const STORE_VERSION: u32 = 1;

/// On-disk layout of `edit-stacks.json`.
#[derive(Serialize, Deserialize)]
struct StackFile {
    version: u32,
    stacks: HashMap<String, Adjustments>,
}

/// Write the full stack map to `path` (pretty JSON; parent dirs created).
pub fn save_stacks_to(path: &Path, stacks: HashMap<String, Adjustments>) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let file = StackFile { version: STORE_VERSION, stacks };
    let json = serde_json::to_string_pretty(&file).context("serializing edit stacks")?;
    fs::write(path, json).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Read the stack map from `path`. A missing file is a first run → empty map.
pub fn load_stacks_from(path: &Path) -> Result<HashMap<String, Adjustments>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let json =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let file: StackFile =
        serde_json::from_str(&json).with_context(|| format!("parsing {}", path.display()))?;
    Ok(file.stacks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Unique temp path per test so parallel test runs don't collide.
    fn temp_store(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("pixagram-test-{}-{name}.json", std::process::id()))
    }

    #[test]
    fn save_load_round_trips() {
        let path = temp_store("roundtrip");
        let mut stacks = HashMap::new();
        stacks.insert(
            "Z:/photos/DSCF0001.RAF".to_string(),
            Adjustments { exposure: 1.5, rotation: 3, ..Default::default() },
        );
        stacks.insert("Z:/photos/DSCF0002.RAF".to_string(), Adjustments::default());

        save_stacks_to(&path, stacks.clone()).expect("save");
        let loaded = load_stacks_from(&path).expect("load");
        fs::remove_file(&path).ok();

        assert_eq!(loaded, stacks, "round-trip must preserve every stack exactly");
    }

    #[test]
    fn missing_file_is_an_empty_map() {
        let path = temp_store("missing");
        let loaded = load_stacks_from(&path).expect("missing file should not error");
        assert!(loaded.is_empty());
    }

    #[test]
    fn partial_json_gets_field_defaults() {
        // A stack saved before `rotation` existed (or any future field) must
        // still load — `#[serde(default)]` fills the gaps.
        let path = temp_store("partial");
        fs::write(
            &path,
            r#"{ "version": 1, "stacks": { "a.RAF": { "exposure": 0.5 } } }"#,
        )
        .expect("write fixture");
        let loaded = load_stacks_from(&path).expect("load");
        fs::remove_file(&path).ok();

        let adj = loaded.get("a.RAF").expect("entry survives");
        assert_eq!(adj.exposure, 0.5);
        assert_eq!(adj.rotation, 0, "absent fields fall back to defaults");
    }
}
