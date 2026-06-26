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
