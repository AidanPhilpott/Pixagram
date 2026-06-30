# Handoff — current state

> Living snapshot for resuming work. **Overwrite this section** at the end of each
> work session so the top of the file is always "where we are right now." Append a
> dated entry to [PROGRESS.md](PROGRESS.md) for the running history, and add any
> durable gotchas to [INSIGHTS.md](INSIGHTS.md).

**Last updated:** 2026-06-29 (session 4)
**Current phase:** Phase 2 (Full adjustment pipeline) — **IN PROGRESS.** Phase 1 is
COMPLETE/color-verified/tagged `v0.1.0`. Step 1 (**contrast**, perceptual S-curve)
is committed (`a9a3beb`), color-verified in the app. Step 2 (**white balance —
temperature/tint**) is built across the three lockstep files; frontend builds, 8/8
Rust tests pass. **Awaiting human verification of the temp/tint sliders before
committing step 2 and starting step 3 (highlights/shadows/whites/blacks).**
**Branch:** `master` (local-only, no remote) · **Last tag:** `v0.1.0`

> **Color-profile / "Fuji recipe" feature → decided: belongs in Phase 3 (Presets).**
> A profile = a saved `Adjustments` value (JSON). Recommended model is **(A)** a
> named, editable bundle of the same sliders (honors the single-shared-definition
> constraint + the "don't reproduce Fuji recipes" non-goal), **not (B)** a base
> LUT/film-sim layer beneath the sliders. Settle A-vs-B when Phase 3 starts. Phase 2
> carry-forward: keep every new field `#[serde(default)]` so profiles save/load for
> free. See "Open risks" + PROGRESS 2026-06-29 (session 4).

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

## ⚠️ START HERE NEXT SESSION — continue Phase 2 (step 3: highlights/shadows)

Steps 1 (contrast) + 2 (white balance) are built. **Next action: human-verify the
temp/tint sliders on a real RAF, commit step 2, then build step 3 — tone region
sliders (highlights / shadows / whites / blacks)** following the same recipe (Rust
`apply_pixel` + GLSL shader + TS mirror + slider, together). Each new op is: add a
field to `Adjustments` (Rust + TS), shader math, one `<AdjustmentSlider>` in App.tsx
wired through `updateField`.

**WB design (decided this session):** temp/tint is a **relative, luminance-preserving
chroma shift on the developed image** (chosen over a true raw Kelvin WB, which would
require re-developing per slider tick and break the live GPU preview). This is how
Lightroom treats non-raw files. The develop's base-color accuracy (single D65 matrix,
illuminant interpolation) is a **separate** develop-side task, unaffected by — and not
fixed by — this slider. See "Open risks".

### What landed in step 2 (white balance) — pending verify + commit
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
