//! Recording: screen (FFmpeg gdigrab, hidden), speaker/mic WAV tracks, click logging,
//! and the "save" step that lines everything up and builds the final files.
use crate::audio::{self, CaptureResult};
use crate::ffmpeg::{self, s};
use crate::jobs::{Done, JobCtl};
use crate::win;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AudioMode {
    Speakers,
    Microphone,
    Both,
    Off,
}
impl AudioMode {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Microphone,
            2 => Self::Both,
            3 => Self::Off,
            _ => Self::Speakers,
        }
    }
    pub fn to_u8(self) -> u8 {
        match self {
            Self::Speakers => 0,
            Self::Microphone => 1,
            Self::Both => 2,
            Self::Off => 3,
        }
    }
    pub fn speakers(self) -> bool {
        matches!(self, Self::Speakers | Self::Both)
    }
    pub fn mic(self) -> bool {
        matches!(self, Self::Microphone | Self::Both)
    }
}

pub const QUALITIES: [(&str, u32, u32); 4] = [
    ("Native (full resolution)", 0, 0),
    ("1080p (1920 x 1080)", 1920, 1080),
    ("720p (1280 x 720)", 1280, 720),
    ("480p (854 x 480)", 854, 480),
];

pub struct RecordRequest {
    pub mode: AudioMode,
    pub speaker_id: Option<String>,
    pub mic_id: Option<String>,
    pub screen: Option<win::RECT>,
    pub cursor: bool,
    pub highlight: bool,
    pub quality: usize,
    pub fps: u32,
    pub out_dir: PathBuf,
    pub keep_raw: bool,
}

pub struct Session {
    stop: Arc<AtomicBool>,
    audio: Vec<(bool, JoinHandle<Result<CaptureResult, String>>)>,
    video: Option<Child>,
    video_start: Arc<Mutex<Option<Instant>>>,
    progress_thread: Option<JoinHandle<()>>,
    clicks: Arc<Mutex<Vec<(Instant, i32, i32)>>>,
    click_thread: Option<JoinHandle<()>>,
    pub started: Instant,
    stamp: String,
    req: RecordRequest,
    screen_file: Option<PathBuf>,
    log_file: PathBuf,
}

fn raw_dir(out: &Path) -> PathBuf {
    out.join("Raw Tracks")
}

/// Even numbers are required by the H.264 encoder.
fn even(v: i32) -> i32 {
    (v / 2 * 2).max(2)
}

pub fn start(req: RecordRequest) -> Result<Session, String> {
    if req.mode == AudioMode::Off && req.screen.is_none() {
        return Err("Choose an audio source or turn on Record Desktop / Screen.".into());
    }
    std::fs::create_dir_all(&req.out_dir).map_err(|e| format!("Cannot use output folder: {e}"))?;
    let stamp = win::local_stamp();
    let merges = req.screen.is_some() && req.mode != AudioMode::Off || req.mode == AudioMode::Both || req.highlight && req.screen.is_some();
    let stem_dir = if merges { raw_dir(&req.out_dir) } else { req.out_dir.clone() };
    std::fs::create_dir_all(&stem_dir).map_err(|e| e.to_string())?;
    let log_file = crate::settings::app_data_dir().join("last_recording_log.txt");
    let stop = Arc::new(AtomicBool::new(false));

    // Screen first (it takes FFmpeg a moment to start), then audio.
    let mut video = None;
    let video_start = Arc::new(Mutex::new(None));
    let mut progress_thread = None;
    let mut screen_file = None;
    if let Some(r) = req.screen {
        let w = even(r.right - r.left);
        let h = even(r.bottom - r.top);
        let file = stem_dir.join(format!("Screen Recording {stamp} (screen only).mp4"));
        let (_, qw, qh) = QUALITIES[req.quality.min(QUALITIES.len() - 1)];
        let vf = if qw > 0 && (w > qw as i32 || h > qh as i32) {
            format!("scale=w={qw}:h={qh}:force_original_aspect_ratio=decrease,scale=trunc(iw/2)*2:trunc(ih/2)*2,format=yuv420p")
        } else {
            "scale=trunc(iw/2)*2:trunc(ih/2)*2,format=yuv420p".to_string()
        };
        let _ = std::fs::create_dir_all(crate::settings::app_data_dir());
        let log = std::fs::File::create(&log_file).map_err(|e| e.to_string())?;
        let mut cmd = ffmpeg::cmd()?;
        cmd.args(["-hide_banner", "-loglevel", "warning", "-y", "-thread_queue_size", "1024", "-f", "gdigrab"])
            .args(["-framerate", &req.fps.to_string(), "-draw_mouse", if req.cursor { "1" } else { "0" }])
            .args(["-offset_x", &r.left.to_string(), "-offset_y", &r.top.to_string(), "-video_size", &format!("{w}x{h}")])
            .args(["-i", "desktop", "-vf", &vf, "-c:v", "libx264", "-preset", "ultrafast", "-tune", "zerolatency", "-crf", "21"])
            .args(["-progress", "pipe:1", "-stats_period", "0.1", "-movflags", "+faststart"])
            .arg(&file)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(log));
        let mut child = cmd.spawn().map_err(|e| format!("Screen capture could not start: {e}"))?;
        // FFmpeg reports how much video it has; the first report tells us when frame 1 was taken.
        if let Some(out) = child.stdout.take() {
            let vs = video_start.clone();
            progress_thread = Some(std::thread::spawn(move || {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    if let Some(v) = line.strip_prefix("out_time_us=") {
                        if let Ok(us) = v.trim().parse::<i64>() {
                            if us > 0 {
                                if let Ok(mut g) = vs.lock() {
                                    if g.is_none() {
                                        *g = Instant::now().checked_sub(Duration::from_micros(us as u64));
                                    }
                                }
                            }
                        }
                    }
                }
            }));
        }
        video = Some(child);
        screen_file = Some(file);
    }

    let mut audio_threads = Vec::new();
    if req.mode.speakers() {
        let f = stem_dir.join(format!("Speakers Audio {stamp}.wav"));
        audio_threads.push((false, audio::spawn_capture(false, req.speaker_id.clone(), f, stop.clone())));
    }
    if req.mode.mic() {
        let f = stem_dir.join(format!("Microphone Audio {stamp}.wav"));
        audio_threads.push((true, audio::spawn_capture(true, req.mic_id.clone(), f, stop.clone())));
    }

    let clicks = Arc::new(Mutex::new(Vec::new()));
    let click_thread = if req.screen.is_some() && req.highlight {
        let (st, cl) = (stop.clone(), clicks.clone());
        Some(std::thread::spawn(move || {
            let mut was_down = false;
            while !st.load(Ordering::Relaxed) {
                let down = win::left_button_down();
                if down && !was_down {
                    if let Some((x, y)) = win::cursor_pos() {
                        if let Ok(mut c) = cl.lock() {
                            c.push((Instant::now(), x, y));
                        }
                    }
                }
                was_down = down;
                std::thread::sleep(Duration::from_millis(10));
            }
        }))
    } else {
        None
    };

    Ok(Session {
        stop,
        audio: audio_threads,
        video,
        video_start,
        progress_thread,
        clicks,
        click_thread,
        started: Instant::now(),
        stamp,
        req,
        screen_file,
        log_file,
    })
}

impl Session {
    /// True if the screen recorder quit on its own (e.g. bad region).
    pub fn video_failed(&mut self) -> Option<String> {
        if let Some(v) = self.video.as_mut() {
            if let Ok(Some(st)) = v.try_wait() {
                let log = std::fs::read_to_string(&self.log_file).unwrap_or_default();
                let last = log.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("").to_string();
                return Some(format!("Screen recorder stopped ({st}). {last}"));
            }
        }
        None
    }

    /// Signals everything to stop. Call `finish` (in a background job) afterwards.
    pub fn signal_stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(v) = self.video.as_mut() {
            if let Some(stdin) = v.stdin.as_mut() {
                let _ = stdin.write_all(b"q");
                let _ = stdin.flush();
            }
        }
    }

    /// Waits for the recorders, then lines up audio and video and writes the final files.
    pub fn finish(mut self, ctl: &JobCtl) -> Result<Done, String> {
        ctl.set_detail("Finishing screen capture...");
        let mut problems: Vec<String> = Vec::new();
        if let Some(mut v) = self.video.take() {
            let t0 = Instant::now();
            loop {
                match v.try_wait() {
                    Ok(Some(st)) => {
                        if !st.success() {
                            let log = std::fs::read_to_string(&self.log_file).unwrap_or_default();
                            let last = log.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("").to_string();
                            problems.push(format!("screen recorder: {st} {last}"));
                        }
                        break;
                    }
                    Ok(None) if t0.elapsed() < Duration::from_secs(20) => std::thread::sleep(Duration::from_millis(100)),
                    _ => {
                        let _ = v.kill();
                        let _ = v.wait();
                        problems.push("screen recorder had to be force-stopped".into());
                        break;
                    }
                }
            }
        }
        if let Some(t) = self.progress_thread.take() {
            let _ = t.join();
        }
        if let Some(t) = self.click_thread.take() {
            let _ = t.join();
        }
        ctl.set_progress(0.1);
        ctl.set_detail("Finishing audio tracks...");
        let mut speakers: Option<CaptureResult> = None;
        let mut mic: Option<CaptureResult> = None;
        for (is_mic, t) in self.audio.drain(..) {
            match t.join() {
                Ok(Ok(r)) => {
                    if is_mic {
                        mic = Some(r)
                    } else {
                        speakers = Some(r)
                    }
                }
                Ok(Err(e)) => problems.push(format!("{} audio: {e}", if is_mic { "microphone" } else { "speaker" })),
                Err(_) => problems.push("audio recorder crashed".into()),
            }
        }
        let video_start = self.video_start.lock().ok().and_then(|g| *g).unwrap_or(self.started);
        let out = self.req.out_dir.clone();
        let stamp = self.stamp.clone();
        let screen = self.screen_file.clone().filter(|p| p.is_file() && std::fs::metadata(p).map(|m| m.len() > 1000).unwrap_or(false));
        let audio_tracks: Vec<&CaptureResult> = [speakers.as_ref(), mic.as_ref()].into_iter().flatten().collect();
        let name_prefix = match self.req.mode {
            AudioMode::Both => "Speakers + Mic",
            AudioMode::Microphone => "Screen + Mic",
            _ => "Screen Recording",
        };
        let mut log = String::new();
        let mut final_file: Option<PathBuf> = None;

        if let Some(screen) = &screen {
            let target = out.join(format!("{name_prefix} {stamp}.mp4"));
            let clicks: Vec<(f64, i32, i32)> = self
                .clicks
                .lock()
                .map(|c| c.iter().map(|(t, x, y)| (t.saturating_duration_since(video_start).as_secs_f64(), *x, *y)).collect())
                .unwrap_or_default();
            let highlight = self.req.highlight && !clicks.is_empty();
            if audio_tracks.is_empty() && !highlight {
                // Screen only: just move it into place.
                if std::fs::rename(screen, &target).is_ok() {
                    final_file = Some(target);
                } else {
                    final_file = Some(screen.clone());
                }
            } else {
                ctl.set_detail("Combining video and audio...");
                let dur = ffmpeg::probe(screen).map(|i| i.duration).unwrap_or(0.0);
                let mut args: Vec<String> = vec!["-y".into(), "-i".into(), s(screen)];
                for a in &audio_tracks {
                    args.extend(["-i".into(), s(&a.path)]);
                }
                let mut filters = Vec::new();
                let mut labels = String::new();
                for (i, a) in audio_tracks.iter().enumerate() {
                    // Line the audio up with the first video frame.
                    let offset = video_start.saturating_duration_since(a.started).as_secs_f64() - a.started.saturating_duration_since(video_start).as_secs_f64();
                    let f = if offset >= 0.0 {
                        format!("[{}:a]atrim=start={offset:.3},asetpts=PTS-STARTPTS[a{i}]", i + 1)
                    } else {
                        let ms = (-offset * 1000.0) as u64;
                        format!("[{}:a]adelay={ms}|{ms},asetpts=PTS-STARTPTS[a{i}]", i + 1)
                    };
                    filters.push(f);
                    labels.push_str(&format!("[a{i}]"));
                }
                let mut maps = vec!["-map".to_string()];
                if highlight {
                    let r = self.req.screen.unwrap_or_default();
                    let src_w = (r.right - r.left).max(1) as f64;
                    let info = ffmpeg::probe(screen).unwrap_or_default();
                    let scale = if info.width > 0 { info.width as f64 / src_w } else { 1.0 };
                    let size = (56.0 * scale).round().max(16.0) as i32;
                    let boxes: Vec<String> = clicks
                        .iter()
                        .take(500)
                        .map(|(t, x, y)| {
                            let xx = ((x - r.left) as f64 * scale) as i32 - size / 2;
                            let yy = ((y - r.top) as f64 * scale) as i32 - size / 2;
                            format!("drawbox=x={xx}:y={yy}:w={size}:h={size}:color=yellow@0.85:t={}:enable='between(t,{:.3},{:.3})'", (size / 10).max(3), t, t + 0.45)
                        })
                        .collect();
                    filters.push(format!("[0:v]{}[v]", boxes.join(",")));
                    maps.push("[v]".into());
                } else {
                    maps.push("0:v:0".into());
                }
                if !audio_tracks.is_empty() {
                    // Make the sound exactly as long as the video (pad with silence or cut the tail).
                    let fit = if dur > 0.0 { format!(",apad=whole_dur={dur:.3},atrim=end={dur:.3}") } else { String::new() };
                    if audio_tracks.len() == 2 {
                        filters.push(format!("{labels}amix=inputs=2:duration=longest:normalize=0{fit}[aout]"));
                    } else {
                        filters.push(format!("{labels}anull{fit}[aout]"));
                    }
                    maps.extend(["-map".into(), "[aout]".into()]);
                }
                args.extend(["-filter_complex".into(), filters.join(";")]);
                args.extend(maps);
                if highlight {
                    args.extend(["-c:v", "libx264", "-preset", "veryfast", "-crf", "21", "-pix_fmt", "yuv420p"].map(String::from));
                } else {
                    args.extend(["-c:v".into(), "copy".into()]);
                }
                if !audio_tracks.is_empty() {
                    args.extend(["-c:a", "aac", "-b:a", "192k"].map(String::from));
                }
                args.extend(["-movflags".into(), "+faststart".into(), s(&target)]);
                let sub = SubProgress { ctl, from: 0.15, to: 0.95 };
                match sub.run(&args, dur) {
                    Ok(l) => {
                        log = l;
                        final_file = Some(target);
                        if !self.req.keep_raw {
                            let _ = std::fs::remove_file(screen);
                        }
                    }
                    Err(e) if e == "Cancelled" => return Err(e),
                    Err(e) => {
                        problems.push(format!("combining failed ({e}); the separate tracks were kept"));
                        let fallback = out.join(format!("Screen Recording {stamp} (no audio).mp4"));
                        if std::fs::rename(screen, &fallback).is_ok() {
                            final_file = Some(fallback);
                        }
                    }
                }
            }
        } else if self.req.screen.is_some() {
            problems.push("no screen video was recorded".into());
        }

        // Audio-only recordings.
        if self.req.screen.is_none() {
            if audio_tracks.len() == 2 {
                ctl.set_detail("Mixing speakers and microphone...");
                let target = out.join(format!("Speakers + Mic {stamp}.wav"));
                let args: Vec<String> = vec![
                    "-y".into(), "-i".into(), s(&audio_tracks[0].path), "-i".into(), s(&audio_tracks[1].path),
                    "-filter_complex".into(), "[0:a][1:a]amix=inputs=2:duration=longest:normalize=0".into(),
                    "-c:a".into(), "pcm_s24le".into(), s(&target),
                ];
                let dur = ffmpeg::probe(&audio_tracks[0].path).map(|i| i.duration).unwrap_or(0.0);
                match (SubProgress { ctl, from: 0.15, to: 0.95 }).run(&args, dur) {
                    Ok(l) => {
                        log = l;
                        final_file = Some(target);
                    }
                    Err(e) => problems.push(format!("mixing failed: {e}")),
                }
            } else if let Some(a) = audio_tracks.first() {
                final_file = Some(a.path.clone());
            }
        }
        // Raw tracks are no longer needed when the user turned that off.
        if !self.req.keep_raw && final_file.is_some() && (screen.is_some() || audio_tracks.len() == 2) {
            for a in &audio_tracks {
                if Some(&a.path) != final_file.as_ref() {
                    let _ = std::fs::remove_file(&a.path);
                }
            }
        }
        let message = match (&final_file, problems.is_empty()) {
            (Some(f), true) => format!("Recording saved: {}", f.file_name().unwrap_or_default().to_string_lossy()),
            (Some(f), false) => format!("Recording saved ({}), with warnings: {}", f.file_name().unwrap_or_default().to_string_lossy(), problems.join("; ")),
            (None, _) => return Err(format!("Recording failed: {}", problems.join("; "))),
        };
        Ok(Done { message, select: final_file, log })
    }
}

/// Maps an FFmpeg run's progress into part of the overall job progress bar.
struct SubProgress<'a> {
    ctl: &'a JobCtl,
    from: f32,
    to: f32,
}
impl SubProgress<'_> {
    fn run(&self, args: &[String], dur: f64) -> Result<String, String> {
        let (from, to) = (self.from, self.to);
        let r = ffmpeg::run(args, dur, self.ctl);
        // ffmpeg::run reports 0..1; squeeze it into from..to afterwards.
        self.ctl.set_progress(if r.is_ok() { to } else { from });
        r
    }
}
