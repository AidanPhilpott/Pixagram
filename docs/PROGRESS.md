# Progress log

Append a dated entry per work session (newest at top). Keep entries short:
what changed, why, and anything that tripped us up. The current-state snapshot
lives in [HANDOFF.md](HANDOFF.md); durable technical gotchas in
[INSIGHTS.md](INSIGHTS.md).

---

## 2026-07-02 (session 6) — Fuji-style effects (grain, color chrome) + styled preset library

User redirect (Phase 4 deferred): grow presets into a library of styled looks,
modeled on Fuji film-sim recipes — specifically Fuji X Weekly's "Reggie's
Portra" (Classic Chrome, H-1 S-1 Color+2, Grain Weak/Small, CC Strong, CCB
Weak, WB +2R/-4B) — with granular Fuji-style effect controls.

- **Four new `Adjustments` fields** (all `#[serde(default)]`, ts-rs regen'd):
  `color_chrome` + `color_chrome_blue` (saturation-proportional darken+densify,
  blue-weighted variant; pipeline step 10 after HSL, mirrored in the shader) and
  `grain` + `grain_size` (monochrome luminance noise, midtone-masked, 1–3px
  cells). Grain is spatial → new lockstep pair `apply_grain` ↔ shader grain
  block, sin-free hash for cross-language reproducibility (see INSIGHTS).
  Constants: CC_LUM 0.12 / CC_SAT 0.10 / GRAIN 0.15 — tunable, must match GLSL.
- **UI:** new collapsible "Effects" inspector section — Grain, Grain Size,
  Color Chrome, Color Chrome Blue sliders ([0,100], no-sign readout).
- **Presets:** 7 new built-ins (11 total): **Classic Portrait** (the
  Portra-400-style recipe translation, non-infringing name), Vivid Slide,
  Muted Chrome, Golden Hour, Cinema Teal, Faded Matte, Grainy Mono.
  Recipe's NR/Sharpening/Clarity/DR have no counterpart ops yet.
- 47 Rust tests (5 new: color chrome ×2, grain ×3) + `npm run build` pass.
  Not yet human-verified in-app.

---

## 2026-07-02 (session 5, wrap-up) — app-verified, tagged v0.3.0

**Human verification passed.** The user ran the app with Fuji RAFs and Sony
A7II ARWs imported side-by-side and confirmed the whole session-5 batch in
one pass: mixed-format batch import, presets (apply/save/batch), zoom,
rotate, view-original, and undo/redo. Tagged **`v0.3.0`**.

**Launch debugging (the time sink, now permanently fixed):** relaunching the
dev app kept failing with EBUSY on files a previously-killed run had open —
`tauri-dev.log`, the Tauri-CLI / rollup `.node` modules, `esbuild.exe`, then
`target/debug/pixagram.exe`. Real diagnosis: (1) an **orphaned esbuild.exe
service process** was still alive, invisible to name-filtered process scans —
found only by scanning `Win32_Process` by *executable path on Z:*; (2) handles
of truly-dead processes lingered NAS-side under the Task Scheduler batch
logon's own SMB session. Fix in `run-dev.cmd`: run Vite + `cargo run` directly
(skip the npm Tauri CLI), `CARGO_TARGET_DIR` on local disk, logs in local
`%TEMP%` — nothing a dev run opens for write lives on the share anymore.
Recovery playbook in INSIGHTS. Two inert NAS-locked leftovers (old
`tauri-dev.log`, old `target/debug/pixagram.exe`) clear on handle expiry.

---

## 2026-07-01 (session 5, consolidation) — Phase 3 + editing UX + viewport + multi-format RAW

Four parallel worktree agents were launched (presets / editing-UX / viewport /
RAW formats); all four hit the account session limit mid-task. One (editing-UX)
had committed complete work; the parent session ported the other three WIP
diffs onto master, finished the unbuilt pieces, and verified everything.
Landed as four commits on master:

- **Editing UX** (agent-complete): per-photo **undo/redo** (Ctrl+Z / Ctrl+Y /
  Ctrl+Shift+Z) with drag coalescing — every stack write funnels through one
  `applyEdit(key, fn)` choke point that pushes a history snapshot unless the
  same key repeats within 400ms, so a slider drag is ONE undo step. **Quick
  before/after**: hold `\` (momentary) or click the "Original" toggle — the
  preview is handed neutral adjustments, stored stacks untouched. **Collapsible
  inspector sections** (Light / Color / Tone Curve / HSL / Presets) with
  localStorage-persisted open state; only Light starts open.
- **Phase 3 presets** (agent-built, parent-integrated): see the entry below for
  the store/UI details. Integration change: `applyPreset` pushes an undo
  snapshot per affected photo (batch apply is one Ctrl+Z per photo), and the
  panel sits in a collapsible "Presets" section.
- **Viewport** (agent ~70%, parent finished): **rotation** as a shared-
  definition field (`rotation: u8` quarter-turns CW, `#[serde(default)]`) —
  geometry, not color: export uses `image` rotate90/180/270
  (`export::apply_rotation`, 4 orientation tests), preview rotates the quad UVs
  + swaps canvas aspect in `PreviewRenderer`. Stage toolbar buttons + `[`/`]`
  keys. **Zoom/pan** (parent-built): cursor-anchored wheel zoom, drag pan,
  −/+/Fit/% toolbar, double-click Fit↔100%, as a CSS transform on the canvas —
  shader untouched; resets on photo/rotation change. **Edit-stack persistence**
  (`store.rs` + `save_stacks`/`load_stacks`): the whole stacks map saved as
  `<app-data>/edit-stacks.json`, loaded at startup, saved debounced 1s — all
  edits incl. rotation survive restarts. Saving is disabled until a load
  succeeds so a failed load can't blank the store.
- **Multi-format RAW** (agent ~60%, parent finished): develop core split into
  CFA-generic `develop_mosaic(DevelopParams)` — Bayer 2×2 (all four phases) and
  X-Trans 6×6 through the same metadata-driven stages; 6 synthetic-mosaic unit
  tests. Thumbnails: sidecar JPG → **embedded preview JPEG** → develop.
  Picker/labels now RAF/NEF/CR2/CR3/ARW/DNG/ORF/RW2/PEF.

**Real-camera validation (user-supplied Sony A7II ARWs):** caught a latent
demosaic overflow — `out_h = round(rh/scale)` rounds up and the last row's
source window ran past the active area; Fuji never crashed only because its
masked borders left slack. Fixed with explicit clamps + regression test (see
INSIGHTS). After the fix all three ARWs develop neutral (near-white R/G, B/G
within ~2%) and the 1616×1080 embedded JPEG extracts for thumbs (ARW's rawler
decoder only implements `full_image` — the sidecar→embedded→develop chain
covers it).

**Verified:** `cargo test` 42/42; `npm run build` clean; develop_check on 3
ARWs. **Not yet app-eyeballed — human run-through needed before tagging
`v0.3.0`.**

---

## 2026-07-01 (session 5, worktree agent) — Phase 3: presets (save/apply/manage)

Phase 3 core built as decided (model **A**: a preset = a named, editable
`Adjustments` JSON bundle — no second color stage, no new color math).

- **Rust store** (`src-tauri/src/presets.rs`): one pretty-printed JSON file per
  user preset (`{version, name, adjustments}`) under `<app-data>/presets/`
  (Tauri path resolver; `com.pixagram.app`). Store functions take the directory
  explicitly, so the 7 new unit tests run against a temp dir and never touch
  real app data. Names are validated (non-empty, ≤64 chars, can't shadow a
  built-in) and sanitized into file names (no traversal). Unreadable preset
  files are skipped with a warning, not fatal. `#[serde(default)]` on
  `Adjustments` means old preset JSON keeps loading as the struct grows
  (covered by a test).
- **Built-ins in code**: Neutral, Punchy, Soft Film, Mono Contrast — flagged
  `builtin: true`, not deletable/renamable/shadowable, always listed first.
- **Commands** in `lib.rs`: `list_presets` / `save_preset` / `delete_preset` /
  `rename_preset`. The `Preset` TS type is ts-rs-generated like the adjustment
  types (`src/adjustments/generated/Preset.ts`).
- **UI** (`src/components/PresetPanel.tsx`, bottom of the inspector): preset
  list (built-in tag on shipped ones), click to apply, "Save current as
  preset…" with an inline name input (no native prompts), inline rename +
  delete for user presets. Errors surface in the existing status line.
- **Batch apply**: the grid now supports Ctrl/Cmd-click (toggle) and
  Shift-click (range from the primary) multi-select — `selectedIds:
  Set<string>` alongside the primary `selectedId` that drives the preview.
  Applying a preset replaces the stack of every selected photo (plain state
  set, deep-cloned per photo so stacks never alias).

**Verified:** `cargo test --lib` (18 math + 7 preset + 4 binding tests);
`npm run build` clean. Not app-eyeballed (worktree sandbox can't launch the
app) — needs a human run-through before tagging `v0.3.0`.

---

## 2026-06-29 (session 4g) — Phase 2 hardening + tag v0.2.0

Phase 2 (full adjustment pipeline) is **complete**. Both hardening items landed and
Phase 2 is tagged `v0.2.0`.

- **ts-rs type codegen** (`b219ec0`): the TS `Adjustments`/`HslBand`/`CurvePoint`
  types are now generated from the Rust structs (`#[derive(TS)]` →
  `src/adjustments/generated/`), so the data shape can't drift (constraint #1).
  `definition.ts` keeps only runtime helpers. `hsl` → `Vec<HslBand>` for a clean
  `Array<HslBand>`. Added `npm run gen:types`. ts-rs export_to resolves relative to
  the source file's dir, so the path is `../../src/adjustments/generated/`.
- **16-bit float preview** (`41d5a48`): develop emits u16; preview ships raw LE-u16
  pixels (base64) instead of an 8-bit PNG; renderer uploads a float RGB texture
  (OES_texture_float + linear, 8-bit fallback) with a manual Y-flip (UNPACK_FLIP_Y
  doesn't apply to typed-array uploads). User confirmed banding on hard pushes is
  gone. Follow-up: larger payload than PNG — consider raw-binary IPC later.
- **Verified:** `cargo test --lib` 21/21 (18 math + 3 bindings); `npm run build`
  clean; 16-bit preview eyeballed in the app (displays, upright, correct color, no
  banding).

**Next:** Phase 3 — presets / color profiles (the day-one feature). Storage + apply +
UI, not new color math. Settle the A-vs-B profile model with the user first.

---

## 2026-06-29 (session 4f) — Phase 2 step 6: tone curve (point/spline editor)

- Committed step 5 (HSL) as `1a76c8d` after user verification.
- **Step 6 op = tone curve** (user chose the point/spline editor over parametric
  region sliders — the latter would overlap the step-3 tone-region controls). New
  `CurvePoint{x,y}` + `curve: Vec<CurvePoint>` on the struct (default identity).
  `Adjustments` is **no longer `Copy`** (Vec) — now `Clone`, passed by `&`.
  - Math: monotonic-cubic Fritsch–Carlson spline, per channel after contrast. Lives
    in Rust `curve_eval` AND TS `evalCurve` (kept identical). 3 unit tests.
  - **Shader pattern differs:** GLSL doesn't re-implement the spline; TS bakes a
    256-entry 8-bit LUT (`bakeCurveLut`) → `LUMINANCE` texture on unit 1, sampled per
    channel. Rust export evaluates the spline directly. Same shape → preview ≈ export
    (only LUT-quantization diff; preview is 8-bit anyway). `uCurveActive` + a seeded
    identity ramp keep the inactive case a no-op.
  - UI: new `CurveEditor` (SVG) — drag/add(≤16)/remove points, endpoints lock x; the
    drawn line uses the same `evalCurve` so drawn == applied. `App.updateCurve`.
- **Verified:** `npm run build` clean; `cargo test --lib` 18/18. Pending human eyeball.

This completes the six core color ops. **Next:** verify → commit → Phase-2 hardening
(ts-rs codegen so the TS mirror can't drift; 16-bit/float preview to stop banding) →
tag `v0.2.0` → Phase 3 (presets/color profiles).

---

## 2026-06-29 (session 4e) — Phase 2 step 5: HSL (per-hue bands)

- Committed step 4 (vibrance/saturation) as `108d5e7` after user verification.
- **Step 5 op = HSL.** New `HslBand {hue,sat,lum}` + `hsl: [HslBand;8]` on the
  struct (8 named hue bands). Math: RGB→HSL, blend the two bands bracketing the pixel
  hue (8-way segment, identical Rust/GLSL), apply hue rotation / sat scale / lightness
  shift gated by `smoothstep(0,0.15,sat)` so neutrals are untouched, HSL→RGB. Rust
  `apply_hsl` ↔ GLSL `applyHsl`. Runs last in the chain.
  - **WebGL1 constraint:** GLSL ES 1.00 — no array constructors, limited dynamic
    indexing. Band blend = 8-way if/else over constant indices into `uHsl*[8]` uniform
    arrays. `uHslActive` uniform skips HSL entirely (incl. the rgb↔hsl round-trip) when
    all bands neutral, matching the Rust early-out → all-zero HSL is exact identity.
- **UI:** chose band-selector layout (user pick): new `HslPanel` with 8 color swatches
  + Hue/Sat/Luminance sliders for the selected band; edited bands get a dot. `App`
  gained `updateHslBand`; `SLIDERS` config retyped to scalar fields only.
- **Verified:** `npm run build` clean; `cargo test --lib` 15/15 (added rgb↔hsl
  round-trip + red-band-targets-reds). Pending human eyeball.

**Next:** verify → commit → step 6 (tone curve — last core op), then Phase-2 hardening
(ts-rs, 16-bit preview), then tag and Phase 3.

---

## 2026-06-29 (session 4d) — Phase 2 step 4: vibrance/saturation

- Committed step 3 (tone regions) as `68bfeb8` after the user verified the four
  sliders in the app.
- **Step 4 op = saturation + vibrance.** Added two fields + sliders. First
  cross-channel op: applied on the whole RGB triple at the end of the perceptual
  chain (after contrast), scaling chroma around each pixel's luma —
  `mix(luma, rgb, (1+sat)·(1+vib·(1−pixelSat)))`. Vibrance's `(1−pixelSat)` weight
  makes it favor muted colors. Rust `apply_saturation` ↔ GLSL `applySaturation`.
  Restructured `apply_pixel` to apply it after the per-channel loop. 2 unit tests.
- **Verified:** `npm run build` clean; `cargo test --lib` 13/13. Pending human eyeball.

**Next:** verify → commit → step 5 (HSL — the big one: RGB↔HSL + per-hue bands +
inspector restructure for a 3-axis × ~8-band grid).

---

## 2026-06-29 (session 4c) — Phase 2 step 3: tone regions

- Committed step 2 (WB) as `261c468` after the user verified temp/tint in the app.
- **Step 3 op = tone regions** (highlights/shadows/whites/blacks). Added four fields
  to the struct/shader/TS mirror + four sliders. Applied per channel on the
  perceptual value after the encode, before contrast: each control owns a
  `smoothstep`-weighted tonal band and adds a lift scaled by `TONE_STRENGTH = 0.2`
  (const both sides). Rust `apply_tone_regions` ↔ GLSL `toneRegions` kept identical.
  3 unit tests (each control acts on its band; endpoints move).
  - Known tradeoffs: per-channel → mild hue shift in very saturated regions; additive
    masked lifts aren't strictly monotonic, so extreme *combined* settings can
    posterize. Acceptable for normal edits; band edges + strength are tunable.
- **Refactor:** the inspector's now-8 sliders are config-driven via a `SLIDERS` array
  in App.tsx (mapped to `<AdjustmentSlider>`), replacing the repetitive JSX blocks.
- **Verified:** `npm run build` clean; `cargo test --lib` 11/11. Pending human eyeball
  of the four tone sliders, then commit.

**Next:** verify tone regions → commit → step 4 (vibrance/saturation).

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
