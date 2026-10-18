//! Edit settings and the FFmpeg commands for exporting, splitting, mixing and timeline rendering.
use crate::ffmpeg::{self, s, MediaInfo};
use crate::jobs::{Done, JobCtl};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditState {
    pub trim_start: f64,
    pub trim_end: f64, // 0 = to the end
    pub gain_db: f32,
    pub speed: f32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub normalize: bool,
    pub reverse_audio: bool,
    pub mono: bool,
    pub bass_db: f32,
    pub treble_db: f32,
    pub noise_reduce: bool,
    pub compressor: bool,
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub grayscale: bool,
    pub flip_h: bool,
    pub flip_v: bool,
    pub rotation: i32,
    pub crop: [i32; 4], // x, y, w, h in source pixels (w = 0 means no crop)
    pub resize: [u32; 2],
    pub mute_audio: bool,
    pub video_fades: bool,
}
impl Default for EditState {
    fn default() -> Self {
        Self {
            trim_start: 0.0,
            trim_end: 0.0,
            gain_db: 0.0,
            speed: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            normalize: false,
            reverse_audio: false,
            mono: false,
            bass_db: 0.0,
            treble_db: 0.0,
            noise_reduce: false,
            compressor: false,
            brightness: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            grayscale: false,
            flip_h: false,
            flip_v: false,
            rotation: 0,
            crop: [0; 4],
            resize: [0; 2],
            mute_audio: false,
            video_fades: true,
        }
    }
}

impl EditState {
    /// Length of the selected part, before speed change.
    pub fn selection(&self, duration: f64) -> (f64, f64) {
        let start = self.trim_start.clamp(0.0, duration.max(0.0));
        let end = if self.trim_end > start { self.trim_end.min(if duration > 0.0 { duration } else { self.trim_end }) } else { duration };
        (start, end.max(start))
    }
    /// Short list of what is switched on, for the Export page.
    pub fn summary(&self) -> Vec<String> {
        let d = EditState::default();
        let mut v = Vec::new();
        if self.trim_start > 0.0 || self.trim_end > 0.0 {
            v.push(format!("Trim {} - {}", clock(self.trim_start), if self.trim_end > 0.0 { clock(self.trim_end) } else { "end".into() }));
        }
        if self.gain_db.abs() > 0.05 {
            v.push(format!("Volume {:+.1} dB", self.gain_db));
        }
        if (self.speed - 1.0).abs() > 0.005 {
            v.push(format!("Speed {:.2}x", self.speed));
        }
        if self.fade_in > 0.0 || self.fade_out > 0.0 {
            v.push(format!("Fades {:.1}s / {:.1}s", self.fade_in, self.fade_out));
        }
        for (on, name) in [
            (self.normalize, "Normalize"),
            (self.reverse_audio, "Reverse audio"),
            (self.mono, "Mono"),
            (self.noise_reduce, "Noise reduction"),
            (self.compressor, "Compressor"),
            (self.grayscale, "Grayscale"),
            (self.flip_h, "Flip horizontal"),
            (self.flip_v, "Flip vertical"),
            (self.mute_audio, "Audio removed"),
        ] {
            if on {
                v.push(name.into());
            }
        }
        if self.bass_db.abs() > 0.05 || self.treble_db.abs() > 0.05 {
            v.push(format!("EQ bass {:+.0} / treble {:+.0} dB", self.bass_db, self.treble_db));
        }
        if self.brightness != d.brightness || self.contrast != d.contrast || self.saturation != d.saturation {
            v.push("Color adjusted".into());
        }
        if self.rotation != 0 {
            v.push(format!("Rotate {}°", self.rotation));
        }
        if self.crop[2] > 0 && self.crop[3] > 0 {
            v.push(format!("Crop {}x{} at {},{}", self.crop[2], self.crop[3], self.crop[0], self.crop[1]));
        }
        if self.resize[0] > 0 && self.resize[1] > 0 {
            v.push(format!("Resize {}x{}", self.resize[0], self.resize[1]));
        }
        v
    }
}

pub fn clock(t: f64) -> String {
    let t = t.max(0.0);
    let total = t as u64;
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

pub struct Format {
    pub label: &'static str,
    pub ext: &'static str,
    pub video: bool,
}
pub const FORMATS: [Format; 9] = [
    Format { label: "MP4 (H.264)", ext: "mp4", video: true },
    Format { label: "MKV (H.264)", ext: "mkv", video: true },
    Format { label: "WebM (VP9)", ext: "webm", video: true },
    Format { label: "GIF (animated)", ext: "gif", video: true },
    Format { label: "MP3 (audio)", ext: "mp3", video: false },
    Format { label: "WAV (audio)", ext: "wav", video: false },
    Format { label: "FLAC (audio)", ext: "flac", video: false },
    Format { label: "M4A / AAC (audio)", ext: "m4a", video: false },
    Format { label: "OGG Vorbis (audio)", ext: "ogg", video: false },
];
pub const QUALITY: [&str; 4] = ["High (Recommended)", "Maximum", "Medium", "Small File"];

/// A free file name: "name (Edited).mp4", "name (Edited 2).mp4", ...
pub fn unique(dir: &Path, stem: &str, tag: &str, ext: &str) -> PathBuf {
    let mut n = 1;
    loop {
        let name = if n == 1 { format!("{stem} ({tag}).{ext}") } else { format!("{stem} ({tag} {n}).{ext}") };
        let p = dir.join(name);
        if !p.exists() {
            return p;
        }
        n += 1;
    }
}

fn audio_codec(ext: &str, quality: usize) -> Vec<String> {
    let kbps = ["256k", "320k", "192k", "128k"][quality.min(3)];
    let v: Vec<&str> = match ext {
        "mp3" => vec!["-c:a", "libmp3lame", "-b:a", kbps],
        "wav" => vec!["-c:a", "pcm_s24le"],
        "flac" => vec!["-c:a", "flac"],
        "m4a" | "mp4" | "mkv" => vec!["-c:a", "aac", "-b:a", kbps],
        "ogg" => vec!["-c:a", "libvorbis", "-q:a", ["6", "8", "5", "3"][quality.min(3)]],
        "webm" => vec!["-c:a", "libopus", "-b:a", ["160k", "256k", "128k", "96k"][quality.min(3)]],
        _ => vec![],
    };
    v.into_iter().map(String::from).collect()
}

/// Builds the FFmpeg arguments for exporting `input` with `edit` applied.
/// Returns (args, expected output seconds).
pub fn export_args(input: &Path, info: &MediaInfo, e: &EditState, fmt: &Format, quality: usize, output: &Path) -> Result<(Vec<String>, f64), String> {
    if fmt.video && !info.has_video {
        return Err("This file has no video. Pick an audio format (MP3, WAV, FLAC, M4A or OGG).".into());
    }
    if !fmt.video && !info.has_audio {
        return Err("This file has no audio track to export.".into());
    }
    if input == output {
        return Err("The output would overwrite the original. Choose a different name.".into());
    }
    let (start, end) = e.selection(info.duration);
    let speed = (e.speed as f64).clamp(0.25, 4.0);
    let length = ((end - start) / speed).max(0.0);
    let mut args: Vec<String> = vec!["-y".into()];
    if start > 0.0 {
        args.extend(["-ss".into(), format!("{start:.3}")]);
    }
    // "-t" before "-i" limits how much of the SOURCE is read, so speed changes keep the trim correct.
    if end > start && (info.duration <= 0.0 || end < info.duration - 0.01) {
        args.extend(["-t".into(), format!("{:.3}", end - start)]);
    }
    args.extend(["-i".into(), s(input)]);

    // Audio effects.
    let mut af: Vec<String> = Vec::new();
    if e.gain_db.abs() > 0.05 {
        af.push(format!("volume={:.2}dB", e.gain_db));
    }
    if e.bass_db.abs() > 0.05 {
        af.push(format!("bass=g={:.2}:f=120", e.bass_db));
    }
    if e.treble_db.abs() > 0.05 {
        af.push(format!("treble=g={:.2}:f=5000", e.treble_db));
    }
    if e.noise_reduce {
        af.push("afftdn=nf=-25".into());
    }
    if e.compressor {
        af.push("acompressor=threshold=0.12:ratio=3:attack=10:release=200".into());
    }
    if e.reverse_audio {
        af.push("areverse".into());
    }
    if (speed - 1.0).abs() > 0.005 {
        let mut r = speed;
        while r > 2.0 {
            af.push("atempo=2.0".into());
            r /= 2.0;
        }
        while r < 0.5 {
            af.push("atempo=0.5".into());
            r /= 0.5;
        }
        af.push(format!("atempo={r:.6}"));
    }
    if e.fade_in > 0.0 {
        af.push(format!("afade=t=in:st=0:d={:.2}", e.fade_in));
    }
    if e.fade_out > 0.0 && length > 0.0 {
        af.push(format!("afade=t=out:st={:.3}:d={:.2}", (length - e.fade_out as f64).max(0.0), e.fade_out));
    }
    if e.normalize {
        af.push("loudnorm=I=-16:TP=-1.5:LRA=11".into());
    }
    if e.mono {
        af.push("aformat=channel_layouts=mono".into());
    }

    if fmt.video {
        let mut vf: Vec<String> = Vec::new();
        let [x, y, w, h] = e.crop;
        if w > 1 && h > 1 {
            vf.push(format!("crop={}:{}:{}:{}", w / 2 * 2, h / 2 * 2, x.max(0), y.max(0)));
        }
        if (speed - 1.0).abs() > 0.005 {
            vf.push(format!("setpts=PTS/{speed:.5}"));
        }
        if e.grayscale {
            vf.push("hue=s=0".into());
        }
        if e.brightness.abs() > 0.005 || (e.contrast - 1.0).abs() > 0.005 || (e.saturation - 1.0).abs() > 0.005 {
            vf.push(format!("eq=brightness={:.3}:contrast={:.3}:saturation={:.3}", e.brightness, e.contrast, e.saturation));
        }
        if e.flip_h {
            vf.push("hflip".into());
        }
        if e.flip_v {
            vf.push("vflip".into());
        }
        match e.rotation {
            90 => vf.push("transpose=1".into()),
            180 => vf.push("hflip,vflip".into()),
            270 => vf.push("transpose=2".into()),
            _ => {}
        }
        if e.resize[0] > 0 && e.resize[1] > 0 {
            vf.push(format!("scale={}:{}", e.resize[0] / 2 * 2, e.resize[1] / 2 * 2));
        }
        if e.video_fades && e.fade_in > 0.0 {
            vf.push(format!("fade=t=in:st=0:d={:.2}", e.fade_in));
        }
        if e.video_fades && e.fade_out > 0.0 && length > 0.0 {
            vf.push(format!("fade=t=out:st={:.3}:d={:.2}", (length - e.fade_out as f64).max(0.0), e.fade_out));
        }
        if fmt.ext == "gif" {
            let (fps, width) = [(15, 960), (20, 1280), (12, 720), (10, 480)][quality.min(3)];
            vf.push(format!("fps={fps},scale='min({width},iw)':-2:flags=lanczos,split[g0][g1];[g0]palettegen=stats_mode=diff[gp];[g1][gp]paletteuse=dither=bayer"));
            args.extend(["-vf".into(), vf.join(","), "-an".into(), "-loop".into(), "0".into()]);
        } else {
            vf.push("scale=trunc(iw/2)*2:trunc(ih/2)*2".into());
            args.extend(["-vf".into(), vf.join(",")]);
            if fmt.ext == "webm" {
                let crf = ["31", "24", "36", "42"][quality.min(3)];
                args.extend(["-c:v", "libvpx-vp9", "-crf", crf, "-b:v", "0", "-row-mt", "1", "-deadline", "good", "-cpu-used", "4"].map(String::from));
            } else {
                let crf = ["20", "16", "24", "28"][quality.min(3)];
                args.extend(["-c:v", "libx264", "-preset", "medium", "-crf", crf, "-pix_fmt", "yuv420p"].map(String::from));
            }
            if e.mute_audio || !info.has_audio {
                args.push("-an".into());
            } else {
                if !af.is_empty() {
                    args.extend(["-af".into(), af.join(",")]);
                }
                args.extend(audio_codec(fmt.ext, quality));
            }
            if fmt.ext == "mp4" {
                args.extend(["-movflags".into(), "+faststart".into()]);
            }
        }
    } else {
        args.push("-vn".into());
        if !af.is_empty() {
            args.extend(["-af".into(), af.join(",")]);
        }
        args.extend(audio_codec(fmt.ext, quality));
    }
    args.push(s(output));
    Ok((args, length))
}

/// Re-encodes the part from `from` to `to` (to = 0 means the end) keeping the original format.
fn piece_args(input: &Path, info: &MediaInfo, from: f64, to: f64, output: &Path) -> Vec<String> {
    let ext = output.extension().and_then(|x| x.to_str()).unwrap_or("mp4").to_ascii_lowercase();
    let mut a: Vec<String> = vec!["-y".into()];
    if from > 0.0 {
        a.extend(["-ss".into(), format!("{from:.3}")]);
    }
    a.extend(["-i".into(), s(input)]);
    if to > from {
        a.extend(["-t".into(), format!("{:.3}", to - from)]);
    }
    if info.has_video && !matches!(ext.as_str(), "mp3" | "wav" | "flac" | "m4a" | "ogg" | "aac") {
        if ext == "webm" {
            a.extend(["-c:v", "libvpx-vp9", "-crf", "30", "-b:v", "0", "-row-mt", "1"].map(String::from));
        } else if ext == "gif" {
            a.extend(["-c:v".into(), "gif".into()]);
        } else {
            a.extend(["-c:v", "libx264", "-preset", "fast", "-crf", "18", "-pix_fmt", "yuv420p"].map(String::from));
        }
    } else {
        a.push("-vn".into());
    }
    if info.has_audio {
        a.extend(audio_codec(if ext == "aac" { "m4a" } else { &ext }, 1));
    }
    a.push(s(output));
    a
}

pub fn split(input: &Path, info: &MediaInfo, at: f64, out_dir: &Path, ctl: &JobCtl) -> Result<Done, String> {
    if at <= 0.05 || (info.duration > 0.0 && at >= info.duration - 0.05) {
        return Err("Move the playhead to where you want to split (not at the very start or end).".into());
    }
    let stem = input.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let ext = input.extension().and_then(|x| x.to_str()).unwrap_or("mp4").to_string();
    let a = unique(out_dir, &stem, "Part 1", &ext);
    let b = unique(out_dir, &stem, "Part 2", &ext);
    ctl.set_detail("Writing part 1...");
    let l1 = ffmpeg::run(&piece_args(input, info, 0.0, at, &a), at, ctl)?;
    ctl.set_detail("Writing part 2...");
    let l2 = ffmpeg::run(&piece_args(input, info, at, 0.0, &b), (info.duration - at).max(0.1), ctl)?;
    Ok(Done {
        message: format!("Split into {} and {}", name(&a), name(&b)),
        select: Some(a),
        log: format!("{l1}\n{l2}"),
    })
}

pub fn name(p: &Path) -> String {
    p.file_name().unwrap_or_default().to_string_lossy().to_string()
}

// ----- Timeline and multitrack -----

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioTrack {
    pub path: String,
    pub start: f64,
    pub gain_db: f32,
    pub muted: bool,
    #[serde(default)]
    pub duration: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VideoClip {
    pub path: String,
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub has_audio: bool,
}

pub fn render_timeline(clips: &[VideoClip], output: &Path, ctl: &JobCtl) -> Result<Done, String> {
    if clips.is_empty() {
        return Err("Add at least one video clip to the timeline.".into());
    }
    let mut args: Vec<String> = vec!["-y".into()];
    let mut filters = Vec::new();
    let mut inputs = String::new();
    let mut total = 0.0;
    for (i, c) in clips.iter().enumerate() {
        if !Path::new(&c.path).is_file() {
            return Err(format!("Clip {} is missing: {}", i + 1, c.path));
        }
        if c.end <= c.start {
            return Err(format!("Clip {} has an Out point before its In point.", i + 1));
        }
        let len = c.end - c.start;
        total += len;
        args.extend(["-ss".into(), format!("{:.3}", c.start), "-t".into(), format!("{len:.3}"), "-i".into(), c.path.clone()]);
        filters.push(format!("[{i}:v]fps=30,scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2,setsar=1,format=yuv420p,setpts=PTS-STARTPTS[v{i}]"));
        if c.has_audio {
            filters.push(format!("[{i}:a]aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo,apad,atrim=duration={len:.3},asetpts=PTS-STARTPTS[a{i}]"));
        } else {
            filters.push(format!("anullsrc=r=48000:cl=stereo,aformat=sample_fmts=fltp,atrim=duration={len:.3},asetpts=PTS-STARTPTS[a{i}]"));
        }
        inputs.push_str(&format!("[v{i}][a{i}]"));
    }
    filters.push(format!("{inputs}concat=n={}:v=1:a=1[vout][aout]", clips.len()));
    args.extend(["-filter_complex".into(), filters.join(";"), "-map".into(), "[vout]".into(), "-map".into(), "[aout]".into()]);
    args.extend(["-c:v", "libx264", "-preset", "medium", "-crf", "20", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"].map(String::from));
    args.push(s(output));
    let log = ffmpeg::run(&args, total, ctl)?;
    Ok(Done { message: format!("Timeline rendered: {}", name(output)), select: Some(output.to_path_buf()), log })
}

pub fn mix_tracks(tracks: &[AudioTrack], output: &Path, ctl: &JobCtl) -> Result<Done, String> {
    let live: Vec<&AudioTrack> = tracks.iter().filter(|t| !t.muted).collect();
    if live.is_empty() {
        return Err("Add at least one track that is not muted.".into());
    }
    let ext = output.extension().and_then(|x| x.to_str()).unwrap_or("wav").to_ascii_lowercase();
    let mut args: Vec<String> = vec!["-y".into()];
    let mut filters = Vec::new();
    let mut labels = String::new();
    let mut total: f64 = 0.0;
    for (i, t) in live.iter().enumerate() {
        if !Path::new(&t.path).is_file() {
            return Err(format!("Track is missing: {}", t.path));
        }
        args.extend(["-i".into(), t.path.clone()]);
        let ms = (t.start.max(0.0) * 1000.0) as u64;
        filters.push(format!("[{i}:a]aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo,volume={:.2}dB,adelay={ms}|{ms}[a{i}]", t.gain_db));
        labels.push_str(&format!("[a{i}]"));
        total = total.max(t.start + t.duration);
    }
    // normalize=0 keeps each track at its own volume (FFmpeg otherwise divides by the track count).
    filters.push(format!("{labels}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0,alimiter=limit=0.98[mix]", live.len()));
    args.extend(["-filter_complex".into(), filters.join(";"), "-map".into(), "[mix]".into()]);
    args.extend(audio_codec(&ext, 0));
    args.push(s(output));
    let log = ffmpeg::run(&args, total, ctl)?;
    Ok(Done { message: format!("Mix exported: {}", name(output)), select: Some(output.to_path_buf()), log })
}

#[derive(Serialize, Deserialize)]
pub struct Project {
    pub media: String,
    pub edit: EditState,
    pub tracks: Vec<AudioTrack>,
    pub clips: Vec<VideoClip>,
}
