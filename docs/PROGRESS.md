# Progress log

Append a dated entry per work session (newest at top). Keep entries short:
what changed, why, and anything that tripped us up. The current-state snapshot
lives in [HANDOFF.md](HANDOFF.md); durable technical gotchas in
[INSIGHTS.md](INSIGHTS.md).

---

## 2026-06-29 (session 4b) — Phase 2 step 2: white balance (temp/tint)

- Committed step 1 (contrast) as `a9a3beb` after the user verified the S-curve and
  the slider-card spacing in the running app.
- **Decision:** WB slider = a **relative, luminance-preserving chroma shift on the
  developed image**, not a true raw Kelvin WB. User chose this (over re-developing
  the RAF per tick) so WB stays in the live GPU pipeline / shared preview==export
  shader — matches how Lightroom treats non-raw files. The develop's base D65-matrix
  accuracy is a separate, still-open develop-side task.
- **Step 2 op = white balance.** Added `temperature` + `tint` to the struct
  (`adjustments.rs`), shader (`preview.frag.ts`), TS mirror (`definition.ts`), and two
  sliders. Applied in linear light between exposure and the encode: per-channel gains
  (strength 0.3, const both sides) + a Rec.709-luminance renormalize so it's pure
  chroma (no brightness shift). Temp = R/B warm-cool; tint = green↔magenta via G.
  Rust `apply_white_balance` ↔ GLSL `applyWhiteBalance` kept identical. 3 unit tests.
- **Verified:** `npm run build` clean; `cargo test --lib` 8/8. Pending human eyeball
  of temp/tint in the app, then commit.

**Next:** verify WB → commit → step 3 (highlights/shadows/whites/blacks).

---

## 2026-06-29 (session 4) — Phase 2 kickoff: contrast + generalized plumbing

Began Phase 2 (full adjustment pipeline). Also settled where the user's
**color-profile / "Fuji recipe"** feature belongs.

- **Color-profile placement → Phase 3 (Presets).** It maps onto the README's
  existing "save the stack as a named preset; apply to one/many; manage a library
  (JSON)" scope. Model **(A)**: a profile = a saved `Adjustments` value — a named,
  editable bundle of the same sliders + built-in defaults + batch-apply. Rejected
  **(B)**: a real LUT / base film-sim layer beneath the sliders (second color stage
  in preview+export; README non-goal). A-vs-B to be confirmed when Phase 3 opens.
  Carry-forward into Phase 2: keep every new field `#[serde(default)]` so profiles
  serialize/deserialize for free. Logged in HANDOFF open-risks + a memory note.
- **Step 1 op = contrast.** Added `contrast: f32` to the canonical struct
  (`adjustments.rs`), the GLSL shader (`preview.frag.ts`), and the TS mirror
  (`definition.ts`) together. UI range [-1,1], shown as [-100,+100]. First shipped a
  linear-light pivot scale; after eyeballing it (crushed shadows / blew highlights)
  **switched to a perceptual S-curve** pivoting at mid-grey 0.5 — positive blends
  toward `smoothstep`, negative compresses toward mid. Applied on the gamma-encoded
  value after the linear→sRGB encode; exposure stays linear. Rust `apply_contrast`
  ↔ GLSL `contrastCurve` kept identical. 3 unit tests.
  - Gotcha: a backtick inside a GLSL `//` comment (`` `apply_contrast` ``) silently
    terminated the shader's template literal and broke the TS parse — keep backticks
    out of the shader string.
- **Enabling refactor (no behaviour change to exposure).** Generalized the preview
  plumbing so the remaining five ops are cheap: `PreviewRenderer.setAdjustments(adj)`
  (whole stack → uniforms) replaces `setExposure`; `Preview` takes an `adjustments`
  prop; `App` has one generic `updateField(field, value)`; `ExposureSlider` →
  reusable `AdjustmentSlider`. Each future op is now field + shader line + slider row.
- **Verified:** `npm run build` clean (tsc + vite, 42 modules); `cargo test --lib`
  all 4 pass. Not yet eyeballed in the running app — that's the next gate.

**Next:** human-verify the contrast slider on a real RAF, then step 2 = white
balance (temp/tint), which also pulls in the deferred illuminant-interpolation work.

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
