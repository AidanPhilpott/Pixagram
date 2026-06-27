@echo off
REM Detached launcher for `tauri dev` so the app outlives Claude's process tree.
setlocal
cd /d "Z:\Projects\Claude Code\Pixagram"
set "PATH=%PATH%;%USERPROFILE%\.cargo\bin"
call npm run tauri dev > "Z:\Projects\Claude Code\Pixagram\tauri-dev.log" 2>&1
endlocal
