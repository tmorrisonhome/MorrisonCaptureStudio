# Morrison Capture Studio v5 — Windows portable Rust application

Build: upload these files into the existing GitHub repository root, then commit and push. GitHub Actions **Windows Portable EXE** runs a Windows Rust compile and bundles FFmpeg and FFplay into the EXE. The EXE extracts the media tools beside itself on first use. It needs a writable directory.

## Recording
* WASAPI speaker loopback, microphone, both as separate WAV tracks, or neither.
* Optional desktop video; full screen or numeric region; cursor on/off; frame rate.
* On stop, retains separate WAV and MP4 captures and creates a combined MP4 for modes that use audio.

## Editing
* File library, waveform, click/shift-click trim marks.
* Video frame preview with seek and timed preview playback; FFplay handles sound while images appear in the window.
* Draggable rectangular crop, speed, volume, mono, reverse, loudness, fades, size changes.
* Split media into two files, single-source export to MP3/WAV/FLAC/M4A/OGG/MP4/MKV/WebM/GIF.
* Multitrack audio mixing with per-track delay, gain, mute and export.
* Multiple video clips in a reorderable timeline, per-clip in/out points and MP4 render at normalized 720p/30 fps. The timeline retains per-clip audio where available and inserts silence for video clips without audio; source videos are preserved.
* Save/reopen the project as JSON containing clip and track settings.

## Build status
The source was reviewed here, but cannot be compiled or runtime-tested in this Linux environment because its Windows Rust toolchain is unavailable. GitHub Actions is the authoritative compiler test. Do not treat compilation success as recording/editing runtime validation. Keep the application folder writable for media tool extraction.

New in this project package: File/Open dialogs, independent audio track mixing, joined video clips with their audio (silent clips receive silent audio tracks), video color/rotation/flip effects, optional click-highlighted screen captures, noise reduction, bass/treble/compression filters, in-window frame playback accompanied by FFplay audio, undo/redo for edit properties, and an editable save/reopen project.

Recording audio from some applications may be restricted by operating-system DRM. Respect others' privacy and copyrights when recording.
