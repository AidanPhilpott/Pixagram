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
     The app lists presets from that directory — restart the app or trigger
     any preset action to see it.
   - **Built-in:** add to `builtin_presets()` in `presets.rs` following the
     "Classic Portrait" pattern (comment documenting the source recipe and the
     mapping), then `cargo test --manifest-path src-tauri/Cargo.toml --lib`
     and `npm run build`.
5. **Report the mapping** — a short table of recipe setting → field value, and
   an explicit list of settings that were ignored (no counterpart ops yet:
   Noise Reduction, Sharpening, Clarity, Dynamic Range, ISO). Recommend the
   user fine-tune by eye on a real photo; these numbers are calibrated
   starting points, not colorimetric conversions.

## Recipe-setting → `Adjustments` mapping

All fields are f32; UI shows ×100. Clamp everything to its documented range.

| Recipe setting | Field(s) | Formula / value |
|---|---|---|
| Highlight (−2…+4) | `highlights` | `0.3 × Highlight` (Fuji + = harder highlights) |
| Shadow (−2…+4) | `shadows` | `−0.3 × Shadow` (Fuji + = deeper shadows → our slider inverts) |
| Color (−4…+4) | `vibrance` | `0.1 × Color`, **on top of** the base sim's saturation offset |
| WB shift (R, B each −9…+9) | `temperature`, `tint` | `temperature = (R − B) / 30`; `tint = (R + B) / 40` (+ = magenta, − = green). Check: +2R/−4B → temp 0.2, tint −0.05 ✓ |
| WB Kelvin (vs ~5500K daylight) | `temperature` | `(K − 5500) / 5000`, added before the shift mapping |
| Grain Effect strength | `grain` | Off 0 · Weak 0.25 · Strong 0.5 |
| Grain Effect size | `grainSize` | Small 0.0 · Large 0.75 |
| Color Chrome Effect | `colorChrome` | Off 0 · Weak 0.5 · Strong 1.0 |
| Color Chrome FX Blue | `colorChromeBlue` | Off 0 · Weak 0.5 · Strong 1.0 |
| Exposure compensation | — | Shooting guidance, not develop — skip by default; mention it in the report |
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

Acros/Monochrome color filters (+Ye/+R/+G) have no true counterpart (our
desaturation runs after HSL lum shifts, which still act on hue — approximate
with HSL `lum` on the filtered bands *before* the −1 saturation, and say so).

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
