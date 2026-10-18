@echo off
setlocal
cd /d "%~dp0"
where cargo >nul 2>&1 || (echo Rust is not installed. Install from https://rustup.rs then rerun. & pause & exit /b 1)
if not exist vendor\ffmpeg.exe (
  echo Downloading FFmpeg (needed to embed screen capture and editing support)...
  powershell -NoProfile -ExecutionPolicy Bypass -Command "$ErrorActionPreference='Stop'; Invoke-WebRequest 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip' -OutFile 'ffmpeg.zip'; Expand-Archive -Force ffmpeg.zip 'ffmpeg-unpacked'; $f=Get-ChildItem 'ffmpeg-unpacked' -Recurse -Filter ffmpeg.exe | Select-Object -First 1; if (!$f) {throw 'ffmpeg.exe missing'}; New-Item -Force -ItemType Directory vendor | Out-Null; Copy-Item $f.FullName vendor\ffmpeg.exe"
  if errorlevel 1 (echo FFmpeg download failed. & pause & exit /b 1)
)
cargo build --release --features bundle-ffmpeg
if errorlevel 1 (echo Build failed. See compiler messages above. & pause & exit /b 1)
echo EXE created at target\release\morrison-capture-studio.exe
pause
