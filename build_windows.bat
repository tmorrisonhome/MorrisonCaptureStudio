@echo off
setlocal
cd /d "%~dp0"
where cargo >nul 2>&1 || (echo Rust is not installed. Install it from https://rustup.rs and run this file again. & pause & exit /b 1)

if exist vendor\ffmpeg.exe goto build
echo Downloading FFmpeg - it gets built into the program...
powershell -NoProfile -ExecutionPolicy Bypass -Command "$ProgressPreference='SilentlyContinue'; $ErrorActionPreference='Stop'; New-Item -Force -ItemType Directory vendor | Out-Null; Invoke-WebRequest 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip' -OutFile 'ffmpeg.zip'; Expand-Archive -Force ffmpeg.zip 'ffmpeg-unpacked'; $f=Get-ChildItem 'ffmpeg-unpacked' -Recurse -Filter ffmpeg.exe | Select-Object -First 1; if (!$f) {throw 'ffmpeg.exe missing'}; Copy-Item $f.FullName vendor\ffmpeg.exe"
if errorlevel 1 (echo FFmpeg download failed. Check your internet connection and try again. & pause & exit /b 1)

:build
cargo build --release --features bundle-ffmpeg
if errorlevel 1 (echo Build failed. See the messages above. & pause & exit /b 1)
echo.
echo Done. Your program is: target\release\morrison-capture-studio.exe
pause
