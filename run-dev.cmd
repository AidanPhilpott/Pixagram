@echo off
REM Detached launcher for the dev app so it outlives Claude's process tree.
REM
REM Everything volatile lives on LOCAL disk, not the Z: NAS share:
REM - Runs Vite + `cargo run` directly instead of `npm run tauri dev` (the npm
REM   Tauri CLI loads a native .node module off the share; a killed run leaves
REM   a stale SMB lock on it that bricks the next launch). Debug builds embed
REM   devUrl (http://localhost:1420) from tauri.conf.json on their own.
REM - CARGO_TARGET_DIR is local: build artifacts (incl. pixagram.exe) on the
REM   share get stale-locked the same way, and local builds are faster anyway.
REM - Logs go to local %TEMP% for the same reason.
setlocal
cd /d "Z:\Projects\Claude Code\Pixagram"
set "PATH=%PATH%;%USERPROFILE%\.cargo\bin"
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\pixagram\target"
if not exist "%CARGO_TARGET_DIR%" mkdir "%CARGO_TARGET_DIR%"
start "pixagram-vite" /b cmd /c "npm run dev > "%TEMP%\pixagram-vite.log" 2>&1"
call cargo run --manifest-path src-tauri\Cargo.toml > "%TEMP%\pixagram-app.log" 2>&1
endlocal
