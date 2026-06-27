# Progress log

Append a dated entry per work session (newest at top). Keep entries short:
what changed, why, and anything that tripped us up. The current-state snapshot
lives in [HANDOFF.md](HANDOFF.md); durable technical gotchas in
[INSIGHTS.md](INSIGHTS.md).

---

## 2026-06-26 — Phase 1 verification + color fix

**Ran the app on real X-T5 RAFs** (via a Windows Task Scheduler launch — the
sandbox reaps Claude's child processes at turn boundaries, so a normal background
launch dies; a scheduled task runs under the Scheduler service and survives).

**Worked:** import (file picker), thumbnails (camera-JPEG sidecars), click→decode,
exposure slider live re-render, export stub message.

**Bug found + fixed — green preview.** Decoded RAFs rendered green. Diagnosed with
a new `examples/diag.rs`: demosaic is correct (sharp), but rawler 0.7.2 doesn't
apply as-shot white balance for X-T5 → green cast. Fixed in `raf::apply_as_shot_wb`
(multiply developed sRGB by `wb_coeffs`, green-normalized). Verified neutral/correct
on 3 frames. See INSIGHTS.

**Also fixed:** reselecting a photo re-decoded the RAF every time → added a
frontend preview cache (`previewCache` in App.tsx) per the brief's caching rule.

**Known/deferred:** drag-and-drop import not implemented (Phase 1 only required the
file picker; DnD is a Phase 2 nicety).

**Next:** confirm the corrected color in the running app, then tag and start Phase 2.

---

## 2026-06-26 — Project kickoff, Phase 1 skeleton

**Goal:** Stand up Phase 1 from [README.md](../README.md) — smallest app that
proves RAF decode, X-Trans color, the Rust↔frontend bridge, and live re-render.

**Done**
- Toolchain: installed Rust (rustup 1.29, stable-msvc) and VS 2022 Build Tools
  (C++ workload) via winget. Node/npm/pnpm and WebView2 were already present.
- Initialized git; added `.gitignore`; marked the network-share repo as a git
  `safe.directory`.
- **Shared adjustment definition** (constraint #1): canonical Rust struct + math
  in `src-tauri/src/adjustments.rs`; TS mirror in `src/adjustments/definition.ts`;
  matching GLSL in `src/gl/preview.frag.ts`. Exposure applied in linear light.
  Added Rust unit tests for the math.
- **Rust/Tauri core:** RAF decode via `rawler` 0.7.2 (`RawDevelop` → sRGB,
  X-Trans/Markesteijn handled internally); downscaled lazy preview; sidecar-JPEG
  thumbnails with develop fallback; export stub; IPC commands `list_images`,
  `decode_preview`, `export_stub`.
- **React UI:** import flow, thumbnail grid, WebGL live preview, exposure slider,
  export button; dark charcoal styling per the visual direction.
- **Verified:** `npm run build` (tsc + vite) passes; Rust core compiles. Generated
  the Tauri icon set from a created `src-tauri/app-icon.png`.

**Decisions**
- **npm, not pnpm** — pnpm's store symlinks fail on the Z: network drive. Updated
  `tauri.conf.json` before-commands to `npm run …`. (See INSIGHTS.)
- Pinned `image = 0.25` to match rawler's re-export so `DynamicImage` unifies.
- Thumbnails prefer the camera JPEG sidecar (user shoots RAW+JPEG) for fast
  browsing; RAF is only decoded for the active preview.

**Tripped us up**
- pnpm symlink failure on Z: drive → switched to npm.
- Tauri v2 serializes command return structs with Rust field names by default —
  added `#[serde(rename_all = "camelCase")]` to the returned structs.

**Next:** Human verification of the live exposure slider on a real RAF, then tag
`v0.1.0` and begin Phase 2 (full adjustment pipeline).
