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

- Pipeline: `rawler::decode_file(path)` → `RawImage`, then
  `rawler::imgop::develop::RawDevelop::default().develop_intermediate(&raw)` →
  `Intermediate`, then `.to_dynamic_image()` → `image::DynamicImage` (sRGB,
  `ImageRgb16` for color). `RawDevelop` picks the demosaic — **Markesteijn for
  X-Trans** — so we don't hand-roll it.
- **rawler 0.7.2 X-T5 color has a residual green/cyan cast — do NOT "fix" it with
  a second white-balance pass.** Hard-won lesson: rawler's `RawDevelop` *does*
  apply the as-shot WB (proved via toggling the `WhiteBalance` step: ON center
  px `[65,51,36]` warm vs OFF `[47,57,7]` green) and the camera color matrix.
  Demosaic is correct (sharp, X-Trans handled). But the brightest near-white target
  (the white mouse) still develops to ~`R75 G102 B112` — red low, blue high = a
  green/cyan cast, worst in the dim midtones. This is a **color-matrix/illuminant
  residual, not a WB error**, so a global per-channel gain can't fix it: applying
  the as-shot `wb_coeffs` a second time double-corrects and pushes highlights
  **magenta** (and clips blue). We tried post-develop WB in gamma space (→magenta),
  linear space (→green), and a `C = cam2rgb·diag(wb)·cam2rgb⁻¹` matrix (→blue);
  all bracket but none nail it because the error isn't a single global gain.
  **Decision: leave rawler's default; do proper color calibration in Phase 2**
  (own black-level/matrix/illuminant, consider auto-WB, or try a newer rawler).
  Diagnostic: `cargo run --example diag -- <file.RAF>` prints wb_coeffs / CFA /
  matrix / channel means / brightest-pixel balance and saves PNGs.
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

## Performance

- `Cargo.toml` sets `opt-level = 2` for all dependencies in the dev profile —
  unoptimized `rawler`/`image` make debug-build decode unbearably slow while
  keeping our own crate debuggable.
