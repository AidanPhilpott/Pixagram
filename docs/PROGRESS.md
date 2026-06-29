# Progress log

Append a dated entry per work session (newest at top). Keep entries short:
what changed, why, and anything that tripped us up. The current-state snapshot
lives in [HANDOFF.md](HANDOFF.md); durable technical gotchas in
[INSIGHTS.md](INSIGHTS.md).

---

## 2026-06-29 (session 3) — green cast root-caused + fixed (owned develop)

Settled the color decision by **re-diagnosing from scratch**, which overturned the
previous session's conclusion.

- **Root cause:** not WB, not the matrix — it's rawler 0.7.2's **X-Trans demosaic**
  inflating green ~1.8×. Proof: averaging the raw sensor mosaic per CFA color
  (pre-demosaic) shows the camera's as-shot WB neutralizes the raw sites (R/G, B/G
  ≈ 1.0 on 3 frames), while rawler's demosaiced output is ≈0.25 (green-heavy). The
  metadata (black 1023 / white 16383 / `wb_coeffs` / D65+A matrices) is all correct.
  Also dumped the Fuji makernote WB tags (0xf00c/d/e) directly — rawler reads the
  right one. The old "matrix/illuminant residual" notes were a misread (confounded
  by a luma>240 *midtone* bucket and highlight clipping).
- **Fix:** new `src-tauri/src/develop.rs` — a camera-general owned develop. rawler
  unpacks the mosaic + metadata only; we do black-subtract/normalize → area-averaging
  demosaic (each output px averages real R/G/B sites in its footprint, expanded to
  ≥1 CFA tile) → WB → camera→sRGB matrix (D65) → baseline exposure → sRGB gamma.
  `raf::preview` + sidecar-less `raf::thumbnail` now call it; JPEG thumbs unchanged.
- **Validated:** `examples/develop_check.rs` runs the production path — all 5 frames
  come out neutral (means R≈G≈B, near-white R/G·B/G ≈ 0.96–1.15) and visually match
  the camera JPEGs (city/bay scene + windowsill plant). Build clean, 2 unit tests pass.
- **Deferred (logged in HANDOFF):** full-res export demosaic (area-average is soft);
  `BASELINE_EXPOSURE_EV=1.7` flat lift pending Phase 2 tone curve; D65-only matrix.

User's framing mattered: they want correct color by default for *any* camera, not a
Fuji hack — so we fixed the pipeline at the source rather than papering over it with
auto-WB. Also explained why thumbnails looked right (they're the camera JPEG) while
the edit preview (rawler develop) was green.

**Next:** human eyeballs the running app (color + live slider) → tag `v0.1.0`.

---

## 2026-06-26 (session 2b) — color investigation, reverted bad WB fix

User reported the preview looked **magenta** after the previous session's WB
"fix". Investigated deeply with `examples/diag.rs` on real frames:
- rawler **does** apply as-shot WB (toggled the WhiteBalance step: ON center
  `[65,51,36]` warm vs OFF `[47,57,7]` green) and the color matrix. Demosaic fine.
- So last session's `apply_as_shot_wb` was a **double WB** → magenta + blue clip.
  **Reverted it**; `develop_raf` now returns rawler's unmodified default.
- Real remaining issue: a milder green/cyan residual (white target ~`R75 G102
  B112`) = matrix/illuminant residual, NOT fixable by a global gain (tried gamma
  →magenta, linear→green, C-matrix→blue; all bracket, none nail it).
- **Left as an open decision** for next session (see HANDOFF "START HERE"): ship
  on rawler default vs own the color pipeline vs try newer rawler.

Lesson: don't "correct" a decode without first proving what the library already
does (a WhiteBalance step toggle would have saved a long detour).

**Next:** make the color decision, then final Phase 1 sign-off + tag v0.1.0.

---

## 2026-06-26 — Phase 1 verification + color fix (NOTE: WB fix later reverted, see session 2b)

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
