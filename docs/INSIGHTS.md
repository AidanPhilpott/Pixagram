# Insights & gotchas

Durable, non-obvious technical findings worth not re-learning. Add to this as the
build grows. (Running history is in [PROGRESS.md](PROGRESS.md); architecture rules
in [../CLAUDE.md](../CLAUDE.md).)

## Environment

- **Use npm, not pnpm.** The repo lives on a network/Z: drive that does not
  support the symlinks pnpm's content-addressed store requires
  (`UNKNOWN: symlink … .pnpm-store`). npm copies instead and works. All
  `tauri.conf.json` before-commands use `npm run …`.
- **Git on a network share** needs `git config --global --add safe.directory …`
  (UNC path `//TheVault/…`) or every git call errors with "dubious ownership."
- **Windows Tauri prerequisites:** Rust (rustup, stable-msvc), the **MSVC C++
  Build Tools** (the linker — installed via VS 2022 Build Tools, `VCTools`
  workload), and the **WebView2 runtime** (already present on Win11). rustc finds
  `link.exe` automatically via the VS registry once Build Tools are installed.

## Raw decode (rawler 0.7.2)

- **Demosaic footprint must be clamped to the visible region.** The output grid
  is `round(r/scale)` per axis, which can round UP — then the last row/column's
  source window `(o+1)·scale` lands one sensor row past the active area. Fuji
  never crashed (masked borders below the frame absorbed the read); the Sony
  A7II's active area runs to the sensor edge → index-out-of-bounds at exactly
  `len`. `develop_mosaic` clamps `sy1/sx1` (and `sy0/sx0`) to `rh/rw`;
  regression test `rounded_up_output_grid_stays_in_bounds`. Any future demosaic
  rewrite must keep this property.
- **Embedded-preview extraction is per-decoder in rawler 0.7.2.** The `Decoder`
  trait defaults `thumbnail_image`/`preview_image`/`full_image` to `None`; ARW
  implements ONLY `full_image` (which returns the embedded JPEG, 1616×1080 on
  the A7II). So always try all three, smallest first (`raf::embedded_jpeg`).
  Don't assume `thumbnail_image` works for a format until tested.
- **`CFA::color_at(row, col)` is absolute-sensor-indexed**, so Bayer phase
  (RGGB vs BGGR vs GRBG vs GBRG) is carried by the pattern itself — sampling
  sites through `color_at` with absolute coordinates handles every phase with
  no special-casing (test: `bayer_all_four_phases_agree`).
- Pipeline: `rawler::decode_file(path)` → `RawImage`, then
  `rawler::imgop::develop::RawDevelop::default().develop_intermediate(&raw)` →
  `Intermediate`, then `.to_dynamic_image()` → `image::DynamicImage` (sRGB,
  `ImageRgb16` for color). `RawDevelop` picks the demosaic — **Markesteijn for
  X-Trans** — so we don't hand-roll it.
- **The green cast was rawler 0.7.2's DEMOSAIC, not white balance — we now own the
  develop (`src/develop.rs`). Earlier "matrix/illuminant residual" notes were WRONG.**
  Definitive diagnosis (see `examples/diag.rs`): averaging the raw sensor mosaic
  per CFA color *before any demosaic* shows the camera's as-shot WB neutralizes the
  raw sites cleanly (R/G, B/G ≈ 1.0 across 3 frames). But rawler's `RawDevelop`
  demosaiced output has R/G ≈ 0.25 — green inflated ~1.8× vs red/blue, because the
  X-Trans 20-of-36 green oversampling isn't normalized. Correct WB applied to those
  unbalanced pixels can never be neutral, so everything renders green. The metadata
  (black level 1023, white 16383, `wb_coeffs`, D65/A color matrices) is all correct
  and self-consistent — only the demosaic is at fault. (Confirmed the Fuji WB tags
  too: rawler reads 0xf00e/`WB_GRBLevels` ≈ the camera's auto WB 0xf00d; those are
  right, they just can't fix a demosaic-distorted image.)
- **Owned develop pipeline (camera-general, `src/develop.rs`):** use rawler ONLY to
  unpack the mosaic (`raw.data`) + read metadata (`blacklevel`, `whitelevel`,
  `wb_coeffs`, `color_matrix`, `camera.cfa`, `active_area`). Then run the canonical
  order ourselves: black-subtract & normalize → demosaic → WB (green-normalized)
  → camera→sRGB matrix (`inv(normalize_rows(xyz2cam · srgb→xyz))`, D65) → baseline
  exposure → sRGB gamma. **Demosaic = area-averaging** (each output pixel averages
  the real R/G/B sites in its footprint, expanded to ≥1 CFA tile so every color is
  present); correct-by-construction and emits the downscaled preview directly.
  Validated to ≈neutral on all 5 frames and visually matched to the camera JPEGs
  via `cargo run --release --example develop_check -- <RAF>`.
- **Two non-obvious gotchas:** (1) `develop`'s area-average is softer than a
  full-res Markesteijn demosaic — fine for the downscaled preview, but EXPORT needs
  a real full-res demosaic (deferred). (2) A pure linear→sRGB develop is ~1.7 EV
  darker than the tone-curved camera JPEG (Fuji also rates ISO low), so
  `develop::BASELINE_EXPOSURE_EV` adds a flat profile-style lift; Phase 2's tone
  curve should replace it. The user's exposure slider stacks on top.
- **CFA `color_at(row,col)` is absolute-sensor-indexed** (`(row+48)%48`), so pass
  absolute coordinates even when iterating inside `active_area`. Index black level
  by `levels[(row%bh)*bw + col%bw]`. `color_at` returns 0=R, 1=G, 2=B (X-Trans site
  counts came out 8:20:8 per 6×6 tile, confirming the mapping).
- Diagnostics: `examples/diag.rs` prints raw-mosaic vs demosaiced channel balance,
  the Fuji makernote WB tags, and per-illuminant renders; `examples/develop_check.rs`
  runs the *production* develop and reports neutrality. Both save ~1000px PNGs.
- **Pin `image` to the version rawler re-exports (0.25).** A mismatched `image`
  version makes the returned `DynamicImage` a *different* type than our crate's,
  and nothing type-checks. The plain `Vec<u8>/Vec<u16>` from `.into_raw()` is the
  version-agnostic escape hatch if a future bump forces divergence.

## Tauri v2

- Command **return** structs serialize with **Rust field names by default**. Add
  `#[serde(rename_all = "camelCase")]` to anything returned to JS, or the TS side
  sees `file_name`/`data_url` instead of `fileName`/`dataUrl`. (Command *argument*
  names are auto-converted camelCase→snake_case, so that direction is free.)
- `capabilities/default.json` must grant `dialog:default` + `dialog:allow-open`
  for the file picker; window label defaults to `main`.
- CSP is set (not `null`) and must allow `img-src … data: blob:` because previews
  and thumbnails travel as base64 `data:` URLs.
- **`app.path().app_data_dir()` needs `use tauri::Manager;`** — the path resolver
  hangs off the `Manager` trait, and the compiler error ("method not found on
  `AppHandle`") doesn't hint at the missing import. Keep path resolution in the
  thin command wrappers (`lib.rs`) and make the store functions take a `&Path`
  so unit tests can inject a temp dir instead of touching real app data.

## Presets (Phase 3)

- **User preset names are validated against the built-ins case-insensitively**
  (can't shadow "Neutral"/"Punchy"/…), and sanitized into file names (anything
  non-alphanumeric except `- _ space` becomes `_`), which also forecloses path
  traversal via the name. The JSON `name` field is the display name; the file
  name is only a derivative — a rename that maps to the same file just rewrites
  in place.
- **`list_presets` skips unreadable preset JSON with a `log::warn`** instead of
  failing the whole panel — one hand-edited broken file must not hide the rest
  of the library.

## Color / preview

- **Phase 1 preview is 8-bit** (PNG `data:` URL → WebGL `UNSIGNED_BYTE` texture).
  Fine for one exposure slider, but pushing exposure hard will band. Before
  stacking many tone operations (Phase 2+), move to a 16-bit/float preview path
  (e.g. half-float texture) to preserve headroom.
- **Exposure is applied in linear light** (sRGB→linear, ×2^EV, linear→sRGB) and
  the Rust and GLSL implementations are kept byte-for-byte equivalent. This is the
  WYSIWYG guarantee — keep it when adding ops.
- Previews are downscaled (longest edge 2048px) **in Rust before IPC**; thumbnails
  to 384px. Never send a full 40MP buffer across the bridge.

## Frontend / UI

- **Pointer capture swallows `dblclick`.** In the `CurveEditor`, calling
  `setPointerCapture` on `pointerdown` (needed so a drag keeps tracking when the
  cursor leaves the point) redirects the follow-up compatibility `click`/`dblclick`
  events to the *capturing* element — so a `onDoubleClick` on the captured target
  never fires. Detect the double-tap manually instead (track last index + timestamp
  in a ref, treat a quick second tap on the same point as the gesture).

## Performance

- `Cargo.toml` sets `opt-level = 2` for all dependencies in the dev profile —
  unoptimized `rawler`/`image` make debug-build decode unbearably slow while
  keeping our own crate debuggable.
