@echo off
REM Detached launcher for the dev app so it outlives Claude's process tree.
REM
REM Runs Vite + `cargo run` directly instead of `npm run tauri dev`: the npm
REM Tauri CLI loads a native .node module, and a killed previous run can leave
REM a stale SMB lock on it (repo lives on a NAS) that fails every next launch.
REM `tauri dev` does nothing else we need — debug builds embed devUrl
REM (http://localhost:1420) from tauri.conf.json on their own.
REM
REM Logs go to local %TEMP%, not the Z: share, for the same stale-lock reason.
setlocal
cd /d "Z:\Projects\Claude Code\Pixagram"
set "PATH=%PATH%;%USERPROFILE%\.cargo\bin"
start "pixagram-vite" /b cmd /c "npm run dev > "%TEMP%\pixagram-vite.log" 2>&1"
call cargo run --manifest-path src-tauri\Cargo.toml > "%TEMP%\pixagram-app.log" 2>&1
endlocal
