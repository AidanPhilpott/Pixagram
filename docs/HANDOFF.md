# Handoff — current state

> Living snapshot for resuming work. **Overwrite this section** at the end of each
> work session so the top of the file is always "where we are right now." Append a
> dated entry to [PROGRESS.md](PROGRESS.md) for the running history, and add any
> durable gotchas to [INSIGHTS.md](INSIGHTS.md).

**Last updated:** 2026-06-29 (end of session 3)
**Current phase:** Phase 1 (Skeleton) — fully functional end-to-end on real X-T5
RAFs. **The green-cast color bug is FIXED** (owned develop pipeline). Awaiting a
human to eyeball the running app, then tag `v0.1.0`.
**Branch:** `master` · **Next tag:** `v0.1.0` (after app sign-off)

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

## ⚠️ START HERE NEXT SESSION — verify the app, then tag v0.1.0

The green-cast bug is fixed (chose original-decision option 2: **own the color
pipeline**, but the root cause turned out to be rawler's *demosaic*, not WB — see
below). Everything compiles, tests pass, and the develop is validated neutral on
all 5 sample frames. **Next action: open the running app, click a RAF, confirm the
edit preview now shows correct color (no green) and the exposure slider still works
live. If good → tag `v0.1.0` and start Phase 2.**

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
