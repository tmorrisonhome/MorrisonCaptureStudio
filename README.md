# Morrison Capture Studio v5.1 — standalone Windows recorder and editor

One portable EXE. FFmpeg is built inside it, and all playback happens **inside the program window**. No outside player opens and no black console windows appear.

## How to build the EXE (GitHub — recommended)

1. In your GitHub repository, **delete the old files**, especially the old `src` folder, because several files were replaced or renamed.
2. Upload **everything in this zip**, keeping the folders. The `.github` folder is hidden on some PCs; make sure `.github/workflows/windows-build.yml` is uploaded too.
3. Commit. The **Windows Portable EXE** workflow starts automatically on pushes to `main` or `master`. You can also start it from the **Actions** tab with **Run workflow**.
4. When it shows a green check, open the run and download **MorrisonCaptureStudio-Windows-Portable**. It contains `morrison-capture-studio.exe`.
5. Copy the EXE anywhere (Desktop, USB stick) and double-click it. Nothing to install.

The first build takes about 10–15 minutes. Later builds are faster because of caching.

## How to build on your own PC (optional)

Install Rust from https://rustup.rs (choose the default options, which include the Microsoft C++ build tools), then double-click `build_windows.bat`. It downloads FFmpeg once and builds `target\release\morrison-capture-studio.exe`.

## Where things are saved

| What | Where |
|---|---|
| Recordings and exports | `Videos\Morrison Capture Studio` (change it in **Settings**) |
| Separate raw tracks (screen-only video, each WAV) | `Videos\Morrison Capture Studio\Raw Tracks` (can be turned off in Settings) |
| Settings, built-in FFmpeg copy, last recording log | `%LOCALAPPDATA%\MorrisonCaptureStudio` |

On first start the program unpacks its built-in FFmpeg (about 100 MB) into `%LOCALAPPDATA%`. This takes a second or two and only happens once per version.

## What it does

**Record**
- Speakers only, microphone only, both, or no audio. Each source is recorded to its own WAV track.
- Desktop/screen: primary monitor, any other monitor, all monitors, or a custom region.
- Cursor on/off, click highlighting, video quality (native / 1080p / 720p / 480p), and 15–60 FPS.
- On **STOP & SAVE**, the audio is lined up with the first video frame and combined into one MP4 with sound. This runs in the background with a progress bar.

**Recordings tab**
- File list with thumbnails, length and size.
- In-app video and audio preview with play/pause, seek, volume and full screen.
- Left/right channel waveform.
- Quick tools: Trim, Volume, Crop, Split and Delete. Delete sends the file to the Recycle Bin.
- Export panel: choose a format and quality, then press **Export Selected**.

**Audio Editor and Video Editor tabs**
- Audio: trim, volume, speed, fades, bass/treble, normalize, noise reduction, compressor, mono and reverse.
- Video: crop by dragging on the picture, rotate, flip, brightness/contrast/saturation, black and white, resize, and remove sound.
- Undo/Redo (Ctrl+Z / Ctrl+Y).

**Edit / Timeline**
- Join several video clips, each with its own In/Out points, into one 1280×720 MP4.
- Multitrack audio mix: slide tracks in time, set each level, mute, and export to WAV, MP3, FLAC or M4A.
- Save/Open project.

**Media Library, Export and Settings pages**
- Media Library: search, filter, rename, show in Explorer, delete, and add files.
- Export page: full export settings, a list of the edits that will be applied, and the FFmpeg log.
- Settings: output folder, keep raw tracks, custom region, and media engine status.

## Limits to know about

- **Windows 10 or 11 only.**
- **The preview plays the original file.** Edits such as volume, crop and color are applied when you export.
- **Long WAV recordings:** a single WAV track is limited to about 3 hours by the WAV format.
- **Protected content:** some apps block capture of their picture or sound (copy protection), and those parts record as black or silent.
- **Audio/video sync** is lined up automatically to within a few hundredths of a second. On a very busy PC, check the first recording.
- **License:** the built-in FFmpeg is GPL-licensed. That's fine for personal use; if you give the EXE to others, include a note that it contains FFmpeg (https://ffmpeg.org).

## For developers

`src/*_stub.rs`, `src/preview_stubs.rs` and the `preview` feature are only used to preview the screen layout on Linux. The Windows build never uses them.
