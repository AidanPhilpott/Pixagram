---
name: fuji-recipe-to-preset
description: Convert a Fujifilm film-simulation recipe (a Fuji X Weekly URL, or pasted camera settings) into a Pixagram preset. Use when the user shares a film-sim recipe, asks to import/ingest/translate a Fuji recipe or film look, or wants a preset that emulates one.
---

# Fuji recipe → Pixagram preset

Translate a Fujifilm film-simulation recipe into the shared `Adjustments`
vocabulary and deliver it as a preset. The canonical field list and ranges live
in [src-tauri/src/adjustments.rs](../../../src-tauri/src/adjustments.rs); the
shipped example of this exact translation is the **"Classic Portrait"** built-in
in [src-tauri/src/presets.rs](../../../src-tauri/src/presets.rs) (from Fuji X
Weekly's "Reggie's Portra") — read both before translating.

## Workflow

1. **Capture the recipe.** If given a URL, WebFetch it and extract every
   setting: base film simulation, Dynamic Range, Highlight, Shadow, Color,
   Noise Reduction, Sharpening, Clarity, Grain Effect (strength + size), Color
   Chrome Effect, Color Chrome FX Blue, White Balance (type + R/B shift), ISO,
   exposure compensation — plus the prose description of the intended look
   (what film it emulates, what subjects it suits). If pasted, parse the text.
2. **Choose a name.** NEVER copy the recipe's name or a trademarked film name
   (Portra, Kodachrome, Tri-X, …). Pick a short descriptive name for the look
   ("Classic Portrait", "Warm Slide"). It must not collide with a built-in
   (case-insensitive) — check `builtin_presets()` for the current list.
3. **Compose the adjustments** = base-sim starting point + recipe deltas
   (tables below). Sum overlapping fields, clamp to each field's range.
4. **Deliver** (ask the user which, if not obvious):
   - **User preset (default):** write one JSON file to
     `%APPDATA%\com.pixagram.app\presets\<Name>.json` (see format below).
     The app lists presets from that directory — restart the app to see it.
   - **Built-in:** add to `builtin_presets()` in `presets.rs` following the
     "Classic Portrait" pattern (comment documenting the source recipe and the
     mapping), then `cargo test --manifest-path src-tauri/Cargo.toml --lib`
     and `npm run build`. **Built-ins live in the Rust binary: the running dev
     app will NOT show them until it's relaunched** (`Start-ScheduledTask
     -TaskName PixagramDev` rebuilds via `cargo run`; Vite hot-reloads only
     the frontend, so new UI + old preset list = stale binary). Tell the user
     this explicitly.
5. **Run the flat/milky guard** (below) before delivering — the most common
   translation failure mode is compounding flatteners into a dead image.
6. **Report the mapping** — a short table of recipe setting → field value, and
   an explicit list of settings that were ignored (no counterpart ops yet:
   Noise Reduction, Sharpening, Clarity, Dynamic Range, ISO). Recommend the
   user fine-tune by eye on a real photo; these numbers are calibrated
   starting points, not colorimetric conversions.

## Flat/milky guard (MANDATORY before delivering)

Flattening operations **compound multiplicatively**: negative `contrast`,
negative `highlights`, positive `shadows`, and a fade curve each look mild
alone, but stacked they grey out every white. (The original "Soft Film"
built-in stacked all four; a 0.95 white landed at ~0.84 and the whole image
read as milky — a user bug report, and why this section exists. A Rust test,
`builtins_keep_whites_bright`, now enforces the floor for built-ins.)

Rules:
- **Map each recipe control to exactly ONE mechanism.** Highlight −1 → the
  `highlights` slider, done. Do NOT also lower the curve ceiling "for film
  feel" — Fuji's Highlight/Shadow are subtle in-camera and the 0.3× mapping is
  already generous.
- **The curve ceiling stays at 1.0** unless the source film is explicitly
  matte/faded (and the recipe or its description says so). A film "toe" is a
  small black-end lift (floor ≤ 0.05); it does not require touching the top.
- **White-point check (do the arithmetic):** trace a 0.95 near-white through
  the pipeline —
  `v += 0.2 × (highlights·w_hl + whites·w_wh)` with `w_hl = smoothstep(0.4,
  0.9, v)`, `w_wh = smoothstep(0.6, 1.0, v)`; then the contrast mix
  (`v + (0.5 + (v−0.5)·0.5 − v)·(−contrast)` for negative contrast); then the
  curve. The result must stay **≥ 0.88** for a normal look, ≥ 0.85 for a
  deliberately matte one. If it doesn't, remove a flattener — don't rescale
  them all.
- Sanity-apply on a real photo (or eyeball the numbers against a comparable
  built-in) before calling the preset done.

## Recipe-setting → `Adjustments` mapping

All fields are f32; UI shows ×100. Clamp everything to its documented range.

| Recipe setting | Field(s) | Formula / value |
|---|---|---|
| Highlight (−2…+4) | `highlights` | `0.3 × Highlight` (Fuji + = harder highlights) |
| Shadow (−2…+4) | `shadows` | `−0.3 × Shadow` (Fuji + = deeper shadows → our slider inverts) |
| Color (−4…+4) | `vibrance` | `0.1 × Color`, **on top of** the base sim's saturation offset |
| WB shift (R, B each −9…+9) | `temperature`, `tint` | `temperature = (R − B) / 30`; `tint = (R + B) / 40` (+ = magenta, − = green). Check: +2R/−4B → temp 0.2, tint −0.05 ✓ |
| WB Kelvin (vs ~5500K daylight) | `temperature` | `(K − 5500) / 5000`, added before the shift mapping |
| WB preset | `temperature`, `tint` | Daylight 0/0 · Shade +0.4/0 · Fluorescent 1 (daylight fl.) +0.2/+0.1 · Fluorescent 2 (warm white) −0.4/+0.1 · Fluorescent 3 (cool white) −0.26/+0.1 · Auto ≈ 0/0. Added before the shift mapping. (Preset K vs 5500K daylight: lower-K preset → cooler render.) |
| Grain Effect strength | `grain` | Off 0 · Weak 0.25 · Strong 0.5 |
| Grain Effect size | `grainSize` | Small 0.0 · Large 0.75 |
| Color Chrome Effect | `colorChrome` | Off 0 · Weak 0.5 · Strong 1.0 |
| Color Chrome FX Blue | `colorChromeBlue` | Off 0 · Weak 0.5 · Strong 1.0 |
| Exposure compensation | `exposure` (conditionally) | Skip by default (shooting guidance). **Include** a partial lift (≈ 0.4–0.6 × the EC midpoint, in EV) when the recipe is explicitly a bright/high-key/overexposed look (e.g. "EC +1 to +2" with Highlight −2) — there the brightness IS the look, and without it the preset reads flat-dim. Say so in the report |
| NR / Sharpening / Clarity / DR / ISO | — | **No counterpart ops.** List as ignored |

## Base film-simulation starting points

Approximations of each sim's character relative to Provia (all-zero). Apply
first, then add the recipe deltas. HSL band order: red 0, orange 1, yellow 2,
green 3, aqua 4, blue 5, purple 6, magenta 7 (`hsl_bands` helper in presets.rs).

| Base sim | Starting point |
|---|---|
| Provia | all zero |
| Velvia | contrast +0.25, vibrance +0.35, saturation +0.1, blacks −0.1 |
| Astia | contrast −0.1, highlights −0.15, vibrance +0.15 |
| Classic Chrome | contrast +0.1, shadows −0.1, saturation −0.25, HSL: blue sat −0.25, red hue +0.1 (toward orange) |
| Classic Neg | contrast +0.2, saturation −0.15, curve fade [(0,0.03),(1,0.97)], HSL: green hue −0.2 (toward yellow), aqua sat −0.15 |
| Pro Neg Hi | contrast +0.15, saturation −0.15 |
| Pro Neg Std | contrast −0.05, saturation −0.2 |
| Eterna | contrast −0.25, saturation −0.35, curve fade [(0,0.05),(1,0.95)] |
| Nostalgic Neg | temperature +0.1, shadows +0.15, saturation −0.1, HSL: orange lum +0.05 |
| Acros | saturation −1.0, contrast +0.15, grain 0.2/size 0.2 (Acros's built-in grain) |
| Monochrome | saturation −1.0 |

Acros/Monochrome color filters (+Ye/+R/+G) **cannot be emulated**: in our
pipeline HSL runs AFTER saturation/vibrance, so once `saturation: -1.0` has
collapsed the pixel to grey the HSL luminance shifts see zero saturation and
do nothing. List the filter as dropped; approximate only the tonal character
(contrast / highlights / shadows / grain).

When the mechanical WB math and the recipe's *described* look disagree
(fluorescent-preset approximations are rough), **the described look wins** —
set temperature/tint by the prose ("warm color cast", "leans green/yellow")
and note the deviation in the report.

## User-preset JSON format

One file per preset, pretty-printed. The on-disk contract is `PresetFile` in
`presets.rs` (`version` currently 1). Partial `adjustments` objects load fine
(serde defaults every field), but write the fields you set explicitly:

```json
{
  "version": 1,
  "name": "Warm Slide",
  "adjustments": {
    "contrast": 0.25,
    "temperature": 0.1,
    "vibrance": 0.3,
    "grain": 0.25,
    "grainSize": 0.0,
    "colorChrome": 0.5,
    "hsl": [ /* if used: exactly 8 {hue,sat,lum} bands, order above */ ],
    "curve": [ {"x": 0.0, "y": 0.03}, {"x": 1.0, "y": 0.97} ]
  }
}
```

Field names are **camelCase** (serde `rename_all`); `curve` needs ≥2 points
sorted by x; the file name should match the sanitized name the store would
produce (alphanumeric, `-`, `_`, space kept; everything else → `_`; `.json`).
Validate by parsing the JSON you wrote; if the Rust side is handy, the store
skips unreadable files with a warning rather than crashing.
