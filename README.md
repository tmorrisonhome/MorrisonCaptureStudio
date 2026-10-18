# Morrison Capture Studio — Rust Edition (Windows)

This is a **Rust Windows desktop application source project**, not a verified prebuilt executable. This Linux authoring environment has no Rust compiler or Windows runtime, so the Windows EXE has **not** been compiled or tested here. The included Windows GitHub Actions workflow and `build_windows.bat` are provided to build it.

## Recording source routing

- **Speakers only (default):** Windows WASAPI loopback on selected playback endpoint. The microphone is never opened.
- **Microphone only:** captures the selected recording device. Speaker audio is not added programmatically, but speakers can leak acoustically into an open microphone.
- **Speakers + microphone:** separate WAV files, plus combined MP4 audio when screen capture is on. Separate stems are retained.
- **No audio:** screen video can record without either audio source.
- Select speaker and microphone endpoints independently. Device routing on Windows depends on the selected endpoint and app output routing.

## Recording

- Desktop screen recording using FFmpeg `gdigrab`, with adjustable FPS, cursor display, and desktop region.
- WASAPI audio capture in 48 kHz 32-bit float stereo WAV.
- Stop saves independent source files to `%USERPROFILE%\Videos\Morrison Capture Studio`.
- MP4 mux for screen video with selected audio; combined mode mixes two independent audio streams.

## Editing and export

- Drag start/end position by clicking the waveform for float WAV files (not all imported WAV formats).
- Time-based trimming, gain, loudness normalization, fades, speed/tempo, mono conversion, reverse audio.
- Video rectangular crop, resize, remove audio, playback speed, GIF conversion.
- Export MP3, WAV, FLAC, M4A, OGG, MP4, MKV, WebM, GIF using FFmpeg.
- Editing does not modify the source file.

**Not yet included:** tested Windows binary, full multitrack editor, video preview, scrub/playback, undo/redo, arbitrary nonlinear multi-clip composition, keyboard shortcuts, dedicated display/region picker. This is a first Rust implementation, not a complete DAW or video NLE.

## Get the portable EXE

### Option A: Build on GitHub Windows runner

1. Create a private GitHub repository and upload the **contents** of this source folder.
2. Open **Actions → Windows Portable EXE → Run workflow**.
3. Wait for successful completion. Download the `MorrisonCaptureStudio-Windows-Portable` artifact.
4. Extract the EXE and launch it on Windows 10/11. On first run it extracts its embedded FFmpeg binary beside itself, so put the EXE in a writeable folder.

GitHub usage and runner limits may apply. Workflow downloads third-party FFmpeg and compiles Rust; it must succeed before a working EXE can be claimed.

### Option B: Build locally on Windows

1. Install Rust MSVC toolchain from https://rustup.rs and Visual Studio C++ Build Tools.
2. Double-click `build_windows.bat` (requires PowerShell and internet access for dependencies).
3. If compilation succeeds, find `target\release\morrison-capture-studio.exe`.

## Licensing and distribution

FFmpeg binary from gyan.dev may be **GPL**; check the license and include appropriate GPL notices/source availability when redistributing a build containing the binary. Dependencies have independent licenses. Users should record only content they are permitted to capture.

## Troubleshooting

- Blank screen recording: inspect FFmpeg availability and Windows screen capture rights. Region coordinates must be on the desktop.
- Empty speaker audio: select the actual output endpoint and start playing sound after record begins.
- Microphone-only recording may contain background sound from the room; headphones can reduce acoustic bleed.
- Some devices will reject 48 kHz float stereo and require format negotiation, not yet implemented.
- Screen recordings require FFmpeg, which is embedded only with the `bundle-ffmpeg` feature.
- FFmpeg and WASAPI start asynchronously; this release does not promise sample-accurate synchronization.
