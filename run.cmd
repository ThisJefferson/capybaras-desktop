@echo off
REM ===================================================================
REM  Capybaras -- development launcher
REM
REM  Double-click this file, or run it from a terminal.
REM  It regenerates the generated assets, then builds and launches the
REM  desktop shell. The FIRST Rust build takes several minutes; after
REM  that it is fast.
REM ===================================================================

setlocal
cd /d "%~dp0"

REM cargo is not always on PATH -- the installer puts it here.
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

echo.
echo  ============================================
echo   Capybaras -- developer launcher
echo  ============================================
echo.

echo  [1/3] Generating design tokens...
call npm run build:tokens
if errorlevel 1 goto fail

echo.
echo  [2/3] Bundling the sidecar...
call npm run build:sidecar
if errorlevel 1 goto fail

echo.
echo  [3/3] Building and launching the shell.
echo        (First run compiles Rust and can take several minutes.)
echo.
cd apps\desktop\src-tauri
cargo run
if errorlevel 1 goto fail

echo.
echo  Capybaras closed normally.
pause
exit /b 0

:fail
echo.
echo  *** SOMETHING FAILED -- scroll up for the error ***
echo.
echo  Common causes:
echo    - Node/npm not installed, or not on PATH
echo    - Rust toolchain missing (needs cargo)
echo    - MSVC C++ build tools missing
pause
exit /b 1
