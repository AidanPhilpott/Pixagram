# Pixagram — working notes for Claude

Non-destructive, Lightroom-style RAW editor for Fujifilm **RAF** files.
Desktop-first (PC → macOS later). The authoritative brief is [README.md](README.md);
**work the phases in order and stop for verification between them.**

> **Start each session here:** read [docs/HANDOFF.md](docs/HANDOFF.md) for current
> state and the next action. Log each session in [docs/PROGRESS.md](docs/PROGRESS.md)
> and record durable gotchas in [docs/INSIGHTS.md](docs/INSIGHTS.md). Update the
> HANDOFF snapshot before you stop.

## Stack
- **Shell:** Tauri v2 (Rust core + web frontend). `src-tauri/` is the Rust crate
  (`pixagram_lib` + thin `main.rs`); the web app is at the repo root (Vite).
- **Frontend:** React + TypeScript, WebGL for the live preview.
- **Raw engine:** `rawler` 0.7.2 — `decode_file` → `RawDevelop::develop_intermediate`
  → `to_dynamic_image()`. rawler handles X-Trans (Markesteijn) demosaic and sRGB
  output. Pinned to `image 0.25` to match rawler's re-export.
- **Package manager:** **npm** (not pnpm — the project lives on a network/Z: drive
  where pnpm's symlinks fail). `tauri.conf.json` before-commands use `npm run …`.

## Two non-negotiable constraints (honor in every phase)
1. **Single shared adjustment definition.** Canonical source of truth is the Rust
   struct + math in [`src-tauri/src/adjustments.rs`](src-tauri/src/adjustments.rs).
   The TS mirror is [`src/adjustments/definition.ts`](src/adjustments/definition.ts)
   and the GPU math is [`src/gl/preview.frag.ts`](src/gl/preview.frag.ts). **All
   three must stay in lockstep** — preview math == export math (WYSIWYG). When you
   add a field/operation, update Rust `apply_pixel`, the shader, and the TS math
   together. The TS **types** are codegen'd from the Rust structs via `ts-rs`
   (`src/adjustments/generated/`); run `npm run gen:types` after changing a struct
   so the data shape can't drift. (Codegen covers the shape only — the shader and
   `apply_pixel` math are still mirrored by hand.)
2. **X-Trans-aware decode + sRGB tagging.** Handled by rawler's `RawDevelop`.

## Performance rules (architecture, not polish)
- Browse via the camera JPEG (sidecar `.JPG`); decode RAF lazily, only for the
  active edit. Downscale previews before they cross IPC — never ship a 40MP buffer.
- Heavy pixel work in Rust; preview compositing on the GPU.

## Commands
- `npm install` — frontend deps.
- `npm run tauri dev` — run the app (launches Vite + Rust, opens the window).
- `npm run build` — type-check + bundle frontend only.
- `cargo build --manifest-path src-tauri/Cargo.toml` — compile the Rust core.
- `cargo test --manifest-path src-tauri/Cargo.toml` — adjustment math unit tests.
- Regenerate icons: `npm run tauri icon src-tauri/app-icon.png`.

## Phase status
- **Phase 1 (skeleton): done, tagged `v0.1.0`.**
- **Phase 2 (full adjustment pipeline): done, tagged `v0.2.0`.** All core color ops
  through the shared definition (exposure, contrast, WB temp/tint, highlights/shadows/
  whites/blacks, vibrance/saturation, per-hue HSL, point/spline tone curve), live GPU
  preview == export, on a 16-bit float preview. TS types are ts-rs-generated from the
  Rust structs (`npm run gen:types`).
- **Phase 3 (presets / color profiles): done, tagged `v0.3.0`, app-verified.**
  JSON preset store + built-ins + batch apply, plus undo/redo, zoom/pan,
  rotation, persistence, multi-format RAW. Session 6 extended it with
  Fuji-style effects (grain, color chrome) and a styled built-in library.
- **Next: Phase 4 leftovers (crop + straighten), then Phase 5 export.**
  See docs/HANDOFF.md.

## Conventions
- Git from commit one; conventional commits; tag releases per phase (`v0.1.0`, …).
- Pin deps; commit lockfiles (`Cargo.lock`, `package-lock.json`).
- **`/fuji-recipe-to-preset` skill** (`.claude/skills/`): converts a Fuji
  film-sim recipe (URL or pasted settings) into a Pixagram preset — mapping
  tables + base-sim starting points live in the skill; the worked example is
  the "Classic Portrait" built-in.
