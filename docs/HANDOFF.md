# Handoff — current state

> Living snapshot for resuming work. **Overwrite this section** at the end of each
> work session so the top of the file is always "where we are right now." Append a
> dated entry to [PROGRESS.md](PROGRESS.md) for the running history, and add any
> durable gotchas to [INSIGHTS.md](INSIGHTS.md).

**Last updated:** 2026-07-02 (session 5 — multi-agent batch, consolidated,
**human-verified**)
**Current phase:** Phase 3 (Presets) — **COMPLETE, APP-VERIFIED, tagged
`v0.3.0`.** The user ran the app with Fuji RAFs and Sony A7II ARWs loaded
side-by-side: batch import of both formats, presets (apply/save/batch), zoom,
rotate, view-original, and undo/redo all confirmed working. Landed as 4
feature commits (`51dae30`…`c015c8e`) plus launcher hardening:
- **Presets (Phase 3, model A):** JSON store under `<app-data>/presets/` with
  list/save/delete/rename commands, 4 built-ins, `PresetPanel` in the
  inspector, Ctrl/Shift grid multi-select + batch apply.
- **Editing UX:** per-photo undo/redo (Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z; every
  stack write funnels through the drag-coalescing `applyEdit` in App.tsx),
  hold-`\` / "Original" toggle before/after view, collapsible inspector
  sections (only "Light" open by default; state in localStorage).
- **Viewport:** cursor-anchored wheel zoom + drag pan + Fit/100% toggle (CSS
  transform on the canvas), and 90° rotation as a shared `Adjustments` field —
  GPU quad-UV rotation kept in lockstep with Rust `export::apply_rotation`.
  Stage toolbar + `[` / `]` keys.
- **Persistence:** per-image edit stacks in `<app-data>/edit-stacks.json`
  (`store.rs`; loaded at startup, saved debounced) — every edit, rotation
  included, survives restarts.
- **Multi-format RAW:** the owned develop is CFA-generic (Bayer 2×2 all phases
  + X-Trans 6×6); thumbnail chain sidecar JPG → embedded preview JPEG →
  develop; picker accepts RAF/NEF/CR2/CR3/ARW/DNG/ORF/RW2/PEF. Validated on
  real Sony A7II ARWs, which exposed + fixed a latent demosaic edge overflow
  (see INSIGHTS "Raw decode").

42 Rust tests + `npm run build` pass. ts-rs types regenerated (now include
`rotation` and `Preset`).
**Branch:** `master` (local-only, no remote) · **Last tag:** `v0.3.0`

> **Color-profile / "Fuji recipe" feature → SHIPPED as Phase 3 presets, model
> (A):** a preset is a named, editable `Adjustments` JSON bundle; a base
> LUT/film-sim layer stays a non-goal. Carry-forward unchanged: every new
> `Adjustments` field keeps `#[serde(default)]` so preset AND edit-stack JSON
> keep loading as the struct grows.

## ⚠️ START HERE NEXT SESSION

Phase 3 is verified and tagged. Remaining phase work, in order:
1. **Phase 4 leftovers: crop + free straighten** (90° rotate is done and
   persists). Both are geometry like rotation — follow its pattern: field(s)
   on `Adjustments` (`#[serde(default)]` + ts-rs), GPU-side geometry in
   `PreviewRenderer`, Rust-side in `export.rs`, lockstep documented in
   `adjustments.rs`.
2. **Phase 5 export:** real JPEG + borders + sRGB tag + batch + naming. Settle
   two open decisions with the user first: **EXIF policy** (preserve / strip /
   preserve-minus-GPS) and the **export-quality demosaic** (area-averaging is
   preview-grade; full-res export wants Markesteijn-class — see Open risks).
3. Then Phase 6 polish (undo/redo shipped early; caching + UI refinement
   remain), Phase 7 macOS.

**Dev launch:** `Start-ScheduledTask -TaskName PixagramDev` → `run-dev.cmd`
(Vite + `cargo run` direct; local `CARGO_TARGET_DIR`; logs in
`%TEMP%\pixagram-*.log`). Do NOT reintroduce share-side logs/builds/CLI —
see INSIGHTS "stale SMB locks".

---

## Where we are

Phase 1 of [README.md](../README.md) is built: a minimal end-to-end app that
proves every hard unknown (RAF decode, X-Trans color, Rust↔frontend bridge, live
GPU re-render). The frontend type-checks and bundles; the Rust core compiles
(6m42s cold) and the adjustment-math unit tests pass.

### Phase 1 checklist (README §"PHASE 1 — Skeleton")
- [x] 1. Tauri app launches on PC *(builds; needs a human to eyeball the window)*
- [x] 2. File picker loads one or many RAFs (`pickRafFiles` → dialog plugin)
- [x] 3. Rust decodes a RAF (X-Trans/Markesteijn) → RGB to frontend (`raf::develop_raf`)
- [x] 4. Thumbnail grid (camera-JPEG sidecar, falls back to downscaled develop)
- [x] 5. Click image → larger preview (`decode_preview` → WebGL)
- [x] 6. One exposure slider via the shared adjustment definition, live re-render
- [x] 7. Stub/log an export step end-to-end (`export_stub`)

### Phase 3 context (superseded — presets shipped in session 5, see top)

The A-vs-B decision resolved as (A); the store, commands, panel UI, and grid
multi-select all exist now. Kept below for the design rationale only.

**WB design (decided session 4):** temp/tint is a **relative, luminance-preserving
chroma shift on the developed image** (chosen over a true raw Kelvin WB, which would
require re-developing per slider tick and break the live GPU preview). This is how
Lightroom treats non-raw files. The develop's base-color accuracy (single D65 matrix,
illuminant interpolation) is a **separate** develop-side task, unaffected by — and not
fixed by — this slider. See "Open risks".

### What landed in Phase 2 hardening (ts-rs + 16-bit preview)
- **ts-rs codegen** (`b219ec0`): `Adjustments`/`HslBand`/`CurvePoint` derive `TS`;
  `cargo test` / `npm run gen:types` writes the TS types to
  `src/adjustments/generated/`. `definition.ts` now imports those + keeps only the
  runtime helpers (defaults, ranges, `HSL_BANDS`). `hsl` became `Vec<HslBand>` so the
  generated type is `Array<HslBand>`. The data shape can no longer drift from Rust.
- **16-bit float preview** (`41d5a48`): develop emits `Rgb<u16>`; the preview ships
  raw LE-u16 pixels (base64) not an 8-bit PNG; the renderer uploads a float RGB
  texture (`OES_texture_float`+linear, 8-bit fallback) with a manual Y-flip. Banding
  on hard tone pushes is gone. **Follow-up:** the raw payload is bigger than the old
  PNG — could move to raw-binary Tauri IPC to drop the base64 overhead.

### What landed in step 6 (tone curve)
- New `CurvePoint { x, y }` type (Rust + TS) and `curve: Vec<CurvePoint>` on
  `Adjustments` (default identity `[(0,0),(1,1)]`). **`Adjustments` is no longer
  `Copy`** (a `Vec` can't be) — it's `Clone`; pass by `&` (already the case).
- Math: a monotonic-cubic (Fritsch–Carlson) spline, applied per channel after
  contrast. The spline lives in Rust `curve_eval` **and** TS `evalCurve` (must stay
  identical) — 3 unit tests (identity no-op, midpoint lift pins endpoints, stays
  monotonic through a steep point).
- **WYSIWYG note (slightly different pattern):** the GLSL shader does *not*
  re-implement the spline. TS bakes the curve into a 256-entry 8-bit LUT
  (`bakeCurveLut`) uploaded as a `LUMINANCE` texture on **unit 1**; the shader samples
  it per channel (HW-linear interp). Rust export evaluates the spline directly. Same
  curve shape → preview ≈ export (tiny LUT-quantization diff only; the preview is
  8-bit anyway). A `uCurveActive` flag + a seeded identity-ramp texture keep the
  inactive case a clean no-op.
- **UI:** new `CurveEditor` (SVG) — drag points, click empty space to add (≤16),
  double-tap to remove; endpoints lock x. The drawn curve uses the same `evalCurve`,
  so what's drawn == what's applied. `App` gained `updateCurve`. Remove uses **manual
  double-tap detection** (a ref tracking last index+time), because `setPointerCapture`
  swallows the native `dblclick` — see INSIGHTS.

### What landed in step 5 (HSL)
- New `HslBand { hue, sat, lum }` type (Rust + TS) and `hsl: [HslBand; 8]` on
  `Adjustments` (8 bands: red/orange/yellow/green/aqua/blue/purple/magenta). Default
  is 8 neutral bands; `#[serde(default)]` preserved so profiles still load.
- Math (Rust `apply_hsl` ↔ GLSL `applyHsl`, identical): RGB→HSL, blend the two bands
  whose centers bracket the pixel hue (segment thresholds hard-coded the same both
  sides), apply hue rotation (`HSL_HUE_DEG=30°`), saturation `×(1+ds)`, lightness
  `±HSL_LUM_STRENGTH=0.5`, all gated by `smoothstep(0,0.15,sat)` so near-neutral
  pixels are untouched; HSL→RGB. Runs last, after saturation/vibrance.
- **WebGL1 note:** GLSL ES 1.00 has no array constructors and limited dynamic
  indexing, so the band blend is an 8-way if/else over **constant** indices into the
  `uHslHue/Sat/Lum[8]` uniform arrays. A `uHslActive` uniform skips the whole HSL
  block (and its rgb↔hsl round-trip) when every band is neutral — matches the Rust
  early-out so all-zero HSL is an exact identity.
- **UI:** new `HslPanel` — a row of 8 color swatches (click to select a band; an edit
  dot marks touched bands) + Hue/Saturation/Luminance sliders for the selected band.
  `App` has an `updateHslBand(index, field, value)` setter; the `SLIDERS` config is
  now typed to the scalar fields only (`ScalarField`) since `hsl` isn't a number.
- 2 unit tests (rgb↔hsl round-trips; red band desaturates reds, leaves blues).

### What landed in step 4 (vibrance/saturation)
- `saturation` + `vibrance` fields (Rust + TS), default 0, UI [-100,+100]. First
  **cross-channel** op: runs on the whole RGB triple at the **end** of the perceptual
  chain (after contrast). Scales each pixel's chroma around its own luma —
  `mix(luma, rgb, factor)` where `factor = (1+saturation)·(1+vibrance·(1−pixelSat))`,
  so vibrance lifts muted colors more than vivid ones. Mirrored in Rust
  `apply_saturation` / GLSL `applySaturation`. 2 unit tests.
- Required restructuring `apply_pixel` to apply this on the full triple after the
  per-channel loop (the earlier ops are per-channel; this one isn't).

### What landed in step 3 (tone regions)
- `highlights` / `shadows` / `whites` / `blacks` fields (Rust + TS), default 0, UI
  [-100,+100]. Applied on the perceptual value **after the encode, before contrast**,
  per channel: each control owns a `smoothstep`-weighted band (highlights 0.4–0.9,
  shadows 0.1–0.6, whites 0.6–1.0, blacks 0.0–0.4) and adds a lift scaled by
  `TONE_STRENGTH = 0.2` (const both sides — must match). Mirrored in Rust
  `apply_tone_regions` / GLSL `toneRegions`. 3 unit tests (each control hits its band,
  endpoints move).
- **Caveat:** per-channel (like contrast), so very saturated regions can shift hue
  slightly; and extreme *combined* settings can posterize (additive masked lifts
  aren't guaranteed monotonic at the limits). Fine for normal edits; revisit if it
  bites. Strengths/band edges are tunable constants.
- **Inspector refactor:** the 8 sliders are now driven by a `SLIDERS` config array in
  App.tsx (`.map` → `<AdjustmentSlider>`), not hand-written blocks.

### What landed in step 2 (white balance)
- `temperature` + `tint` fields (Rust + TS), default 0, UI [-100,+100]. Applied in
  **linear light, between exposure and the encode**, as a luminance-preserving chroma
  shift: per-channel gains (`WB_TEMP_STRENGTH`/`WB_TINT_STRENGTH` = 0.3, duplicated as
  consts both sides — must match) then a Rec.709-luminance renormalize so WB changes
  only color, never brightness. Mirrored in Rust `apply_white_balance` and GLSL
  `applyWhiteBalance`. 3 unit tests (luma preserved; warm favors red; tint moves
  green↔magenta).
- Temperature warms (R↑/B↓) / cools (reverse); tint moves green↔magenta via the green
  channel + the luma renorm. Strength 0.3 is tunable.

### What landed in step 1 (contrast)
- `contrast: f32` field added to `Adjustments` (Rust + TS mirror), default 0, UI
  range [-1,1] shown as [-100,+100]. **Perceptual S-curve** pivoting at mid-grey
  0.5: positive blends toward `smoothstep` (steeper mid-tones), negative compresses
  toward mid. Applied on the gamma-encoded value *after* the linear→sRGB encode
  (exposure stays in linear light). Mirrored in Rust `apply_contrast` and GLSL
  `contrastCurve` — **edit both together.** 3 unit tests (positive pushes from mid,
  full negative compresses without collapsing, mid-grey is a fixed point).
- Switched from the initial linear-pivot scaling to the S-curve this session (it
  crushed shadows / blew highlights). The pipeline doc-order in `adjustments.rs` was
  updated to match (contrast is now step 4, on the perceptual value).
- **Plumbing generalized** (enabling refactor, not a new op): `PreviewRenderer` now
  takes the whole stack via `setAdjustments(adj)` instead of `setExposure`; `Preview`
  takes an `adjustments` prop; `App` has one generic `updateField(field, value)`
  setter; `ExposureSlider` replaced by a reusable `AdjustmentSlider`.
- Preview is still **8-bit** — the S-curve shoulder/toe will band a little under
  hard pushes until the float-preview step (Phase 2 item 3). Shape is right; defer
  polish until then.

### Phase 1 context (still relevant)
The green-cast bug was fixed by owning the color pipeline (root cause: rawler's
*demosaic*, not WB — details below). The Phase-2 color follow-ups (full-res export
demosaic, tone curve to replace `BASELINE_EXPOSURE_EV`, illuminant interpolation)
are under "Open risks." Context on the develop fix:

**What was actually wrong (corrected diagnosis — supersedes old notes):**
- The cast was rawler 0.7.2's **X-Trans demosaic** inflating green ~1.8× (the
  20-of-36 green oversampling isn't normalized). The camera metadata (black/white
  levels, as-shot WB, color matrix) is all CORRECT — the camera's own WB
  neutralizes the *raw mosaic* perfectly; rawler's demosaic is what breaks it.
- Old "matrix/illuminant residual, not a WB error" notes were wrong (the
  brightest-pixel test was confounded by clipping). Fully re-diagnosed by reading
  the raw sensor sites directly. Details in INSIGHTS.

**The fix (`src-tauri/src/develop.rs`, camera-general — not Fuji-specific):**
use rawler only to unpack the mosaic + read metadata, then own the develop:
black-subtract/normalize → area-averaging demosaic → WB → camera→sRGB matrix (D65)
→ baseline exposure (`BASELINE_EXPOSURE_EV`) → sRGB gamma. `raf::preview` and the
sidecar-less `raf::thumbnail` fallback both call it. Camera-JPEG thumbnails are
unchanged. Validate any color change with
`cargo run --release --example develop_check -- <RAF>` (runs the real pipeline,
prints neutrality, saves `check_<stem>.png`); `examples/diag.rs` is the deeper
metadata/mosaic harness.

**Known follow-ups created by this fix (not blockers for v0.1.0):**
- Preview demosaic is area-averaging (softer than Markesteijn). **Export needs a
  real full-res demosaic** — deferred.
- `BASELINE_EXPOSURE_EV = 1.7` is a flat lift standing in for a tone curve; Phase 2
  tone curve should replace it. Single D65 matrix (no illuminant interpolation yet).

**Launch note:** inside Claude's sandbox, background launches get reaped at turn
boundaries — launch via Windows Task Scheduler (`Start-ScheduledTask -TaskName
PixagramDev`, runs `run-dev.cmd`). Outside the sandbox, `npm run tauri dev` works.

## Resume cheat-sheet
- Architecture & rules: [../CLAUDE.md](../CLAUDE.md)
- `npm run build` — frontend type-check + bundle
- `cargo build --manifest-path src-tauri/Cargo.toml` — Rust core
- `cargo test --manifest-path src-tauri/Cargo.toml` — adjustment-math tests

---

## What's next — Phase 2 (do one at a time, verify each)

Full adjustment pipeline on the shared definition + GPU preview + core color
sliders. Concretely, the first moves:

1. **Grow the shared definition** ([adjustments.rs](../src-tauri/src/adjustments.rs))
   one operation at a time — contrast, then white balance (temp/tint), then
   highlights/shadows/whites/blacks, vibrance/saturation, HSL, tone curve. For
   **each** field: update Rust `apply_pixel`, the GLSL shader
   ([preview.frag.ts](../src/gl/preview.frag.ts)), and the TS mirror
   ([definition.ts](../src/adjustments/definition.ts)) **together**, and add a
   slider. Keep the linear-light pipeline order documented in `adjustments.rs`.
2. **Harden the single-source-of-truth:** adopt `ts-rs` so the TS `Adjustments`
   type is generated from the Rust struct (kills the hand-mirror drift risk).
3. **Preview precision:** Phase 1 ships an 8-bit PNG preview. Before stacking many
   tone ops, move to a 16-bit or float preview path to avoid banding when pushing
   adjustments (see INSIGHTS).

## Open risks / decisions to make
- **Color profiles / "Fuji recipes" → Phase 3 (Presets).** Decided this session: a
  profile is a saved `Adjustments` JSON value. Build model **(A)** — a named,
  editable bundle of the existing sliders + a handful of built-in defaults + "save
  current as profile" + batch-apply — which is exactly the README's Phase 3 preset
  scope. Reject/deferred: **(B)** a true LUT / base film-sim layer beneath the
  sliders (adds a 2nd color stage to preview+export; the README lists reproducing
  Fuji recipes as a non-goal). Settle A-vs-B explicitly when Phase 3 opens. Profiles
  are only as expressive as the Phase 2 slider vocabulary, so finish the WB / tone /
  HSL ops first. **Carry-forward: keep every new `Adjustments` field
  `#[serde(default)]` so old profile JSON keeps loading as the struct grows.**
- **EXIF policy on export** (Phase 5): preserve / strip / preserve-minus-GPS — not
  yet decided. Flag to the user when Phase 5 starts.
- **Export-quality demosaic.** The owned develop uses area-averaging (great for the
  downscaled preview, too soft for full-res export). A real full-res demosaic
  (Markesteijn for X-Trans, or fix/replace rawler) is needed before export ships.
- **Baseline exposure vs tone curve.** `BASELINE_EXPOSURE_EV` is a flat stand-in;
  Phase 2's tone curve should replace it. Also no illuminant interpolation yet
  (single D65 matrix) — revisit when adding the WB temp/tint slider.
- **Drag-and-drop import** not implemented yet (Phase 1 needed only the picker).
  Easy add in Phase 2 via Tauri's file-drop event.
- **Preview is 8-bit** — see INSIGHTS; move to 16-bit/float before stacking tone
  ops in Phase 2.
