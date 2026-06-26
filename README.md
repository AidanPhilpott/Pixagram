# RAW Editor — Project Brief

A non-destructive, Lightroom-style photo editor for Fujifilm RAF raw files.
Desktop app, **PC first, macOS next, iPad a separate long-term frontend.**
Priorities, in order: **correct color → fast & light → polished UI.**

This document is the source of truth for the build. Work the phases in order.
Do not sprint ahead — finish a phase, let it be verified, then continue.

---

## Product summary

Import single or many photos into a working set, adjust color (manual sliders
or a saved preset), rotate/crop, and batch-export to JPEG with an optional
white border that pads the image to a target aspect ratio. Originals are never
modified; all edits live as a non-destructive adjustment stack.

The user shoots **RAW + JPEG** on a Fujifilm X-T5, so every shot is a `.RAF`
(raw) plus a `.JPG` (camera render). Exploit this: use the embedded/sidecar
JPEG for fast browsing, decode the RAF only when editing.

---

## Tech stack (chosen — do not substitute without flagging)

- **Shell:** Tauri (Rust core + web frontend). Cross-compiles PC → macOS with no
  rewrite. iPad is explicitly **not** a Tauri target — see Non-goals.
- **Frontend:** React + a WebGL canvas for the live preview.
- **Image engine (Rust):** `rawler` or `libraw` bindings for RAF decode, with
  **X-Trans (Markesteijn) demosaicing** — not generic Bayer, or X-T5 files
  render mushy. Heavy pixel work stays in Rust; live preview runs on the GPU.
- **State / presets:** plain JSON. Portable, diffable, syncable.

### Don't reinvent the wheel
Use established libraries: `rawler`/`libraw` for decode, a known demosaic
implementation, a maintained WebGL/color library for the preview math, Tauri's
own updater for releases. The **only** custom-owned core piece is the shared
adjustment definition below.

---

## Two non-negotiable constraints (expensive to retrofit — honor from day one)

1. **Single shared adjustment definition.** One spec describes an edit (exposure,
   contrast, WB, HSL, curve, rotation, crop, border, etc.). BOTH the GPU preview
   and the Rust export read from it, applying identical math. They must never
   diverge — what you see is what you export.

2. **X-Trans-aware decode.** Target Markesteijn demosaicing for the X-T5 sensor.
   Tag exported JPEGs as **sRGB** or images look wrong everywhere.

---

## Performance rules (this is architecture, not polish)

- Browse via the camera JPEG; decode RAF lazily, only for the active edit.
- Cache decoded previews/thumbnails; don't re-decode on every interaction.
- Never hold full-resolution 40MP buffers for every photo in memory at once.
- All heavy pixel operations in Rust; preview compositing on the GPU.

---

## Feature scope (full target, built across phases)

**Import & workflow** — drag-drop or file picker, single or batch; thumbnail
grid of the working set; non-destructive adjustment stack per photo.

**Editing** — white balance (temp/tint), exposure, contrast,
highlights/shadows/whites/blacks, vibrance/saturation, per-channel HSL, tone
curve; rotate (90° steps + free straighten); crop; undo/redo + reset.

**Presets** — save the current stack as a named preset; apply to one or many
photos; manage a preset library. Stored as JSON.

**Export** — JPEG with quality control; sRGB profile tagged; EXIF policy
(decide: preserve / strip / preserve-minus-GPS); batch export across a
selection; configurable output naming, destination, and collision handling.

**Borders (export-time)** — pad the image to a target aspect ratio with white
fill:
- *No border* (default): native, untouched.
- *Landscape border*: pad to a landscape ratio (default 3:2). A portrait photo
  gets white bars left/right.
- *Portrait border*: pad to a portrait ratio (default 4:5). A landscape photo
  gets white bars top/bottom.
- Target ratios are tunable constants, not architecture.

---

## Visual direction

CleanMyMac-adjacent depth and polish, but **less aggressive** — drop the heavy
multi-stop gradients and neon glow. Dark neutral charcoal workspace so the UI
never biases color perception; the photo is the only saturated thing on screen.
One restrained accent color for active controls. Depth from soft shadows and a
single low-contrast gradient, not sweeps. Premium feel comes from precise
spacing and good typography, not gradient intensity.

---

## PHASE 1 — Skeleton (build this first, then STOP)

Goal: prove every hard unknown end-to-end with the smallest possible app.

1. Tauri app launches on PC.
2. File picker loads one or many RAFs.
3. Rust decodes a RAF (X-Trans / Markesteijn) → RGB buffer to the frontend.
4. Thumbnail grid of decoded images (camera JPEG is fine for thumbs).
5. Click an image → larger preview.
6. **One slider (exposure)**, wired through the shared adjustment definition,
   re-rendering the preview live.
7. Stub/log an export step to confirm the pipeline reaches end-to-end.

**Stop when this runs and show the exposure slider working before adding
anything.** If it runs, raw decode, X-Trans color, the Rust↔frontend bridge,
and live re-render are all proven; everything after is additive.

---

## Later phases (one at a time, verify each)

2. Full adjustment pipeline (shared definition) + GPU preview + core color sliders
3. Presets: save / apply / batch
4. Rotate + crop
5. Export: JPEG + borders + sRGB + EXIF + batch + naming
6. Polish: undo/redo, caching, UI refinement
7. macOS build (config, not rewrite — settle raw-lib linking here)

---

## Engineering practices

- **Git from commit one.** Conventional commits. Tag releases per phase
  (`v0.1.0`, `v0.2.0`, …).
- **Pin dependencies; commit lockfiles** (Cargo.lock, package-lock/pnpm-lock) so
  PC and macOS builds are reproducible.
- Settle the cross-platform raw-decode **build/link approach early** so macOS is
  configuration, not surgery.
- Keep the core as a **portable engine + thin UI** so a future iPad frontend can
  reuse the adjustment definitions and preset JSON.

---

## Non-goals (for now)

- **iPad / iPadOS:** not a Tauri target. A separate native/web frontend, much
  later, reusing the shared adjustment spec and preset JSON. Do **not** let the
  iPad goal distort the desktop architecture.
- Matching Adobe's exact color science or reproducing in-camera Fuji film
  recipes. Aim for excellent, not byte-identical to Lightroom.
- Cloud sync, catalog/library management, AI features.
