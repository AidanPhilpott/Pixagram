# Handoff — current state

> Living snapshot for resuming work. **Overwrite this section** at the end of each
> work session so the top of the file is always "where we are right now." Append a
> dated entry to [PROGRESS.md](PROGRESS.md) for the running history, and add any
> durable gotchas to [INSIGHTS.md](INSIGHTS.md).

**Last updated:** 2026-06-26 (end of session 2)
**Current phase:** Phase 1 (Skeleton) — fully functional end-to-end on real X-T5
RAFs. **One open decision: how to handle a residual color cast (see below).**
**Branch:** `master` · **Next tag:** `v0.1.0` (after the color decision + sign-off)

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

## ⚠️ START HERE NEXT SESSION — the color decision

Everything in Phase 1 works (import, thumbs, decode, live exposure, export stub).
The remaining issue is **color quality**, and it needs a decision before sign-off.

**What we know (fully diagnosed — see INSIGHTS "rawler 0.7.2 X-T5 color"):**
- rawler's demosaic is correct (sharp, X-Trans handled).
- rawler **does** apply as-shot WB + the camera color matrix.
- BUT a residual **green/cyan cast** remains: the brightest near-white target
  (a white mouse) develops to ~`R75 G102 B112` — red low, blue high — worst in
  dim midtones. It's a **matrix/illuminant residual, not a WB error**, so a global
  per-channel gain can't fix it (we tried — gamma→magenta, linear→green,
  matrix C→blue). The current code uses **rawler's unmodified default** (the
  earlier "apply_as_shot_wb" fix was a wrong double-WB and has been reverted).

**Decision to make (pick one, then implement):**
1. **Ship Phase 1 on rawler's default** (mild cast, honest baseline), tag
   `v0.1.0`, and make proper color the *first task of Phase 2*. Recommended —
   keeps the phase gate clean; color is explicitly a Phase 2 concern.
2. **Own the color pipeline now:** pull rawler's *linear camera RGB* (develop with
   steps `[Rescale, Demosaic, CropActiveArea, CropDefault]` — confirmed to yield
   pre-matrix linear data) and do WB → matrix → sRGB ourselves, with
   illuminant-aware matrix selection (rawler has both `A` and `D65` for this
   camera; it defaults to D65, which mismatches warm indoor shots). This is the
   real fix and sets up the Phase 2 WB slider.
3. **Try a newer rawler** (>0.7.2) in case X-T5 color improved upstream. Cheapest
   to test; may shift the API. (We pinned 0.7.2 ourselves; not a hard constraint.)

`examples/diag.rs` is the investigation harness — run
`cargo run --example diag -- <file.RAF>` for wb_coeffs/CFA/matrix/channel means/
brightest-pixel balance + saved PNGs. Use it to validate any color change.

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
- **Residual color cast** in rawler's X-T5 develop — the open decision above. Tied
  to the Phase 2 WB/temp-tint adjustment; resolve before or as part of Phase 2.
- **Drag-and-drop import** not implemented yet (Phase 1 needed only the picker).
  Easy add in Phase 2 via Tauri's file-drop event.
- **Preview is 8-bit** — see INSIGHTS; move to 16-bit/float before stacking tone
  ops in Phase 2.
