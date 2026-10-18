//! Everything that talks to FFmpeg. Every FFmpeg process runs hidden (no console window).
use crate::jobs::JobCtl;
use std::io::{BufRead, BufReader, Read};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// CREATE_NO_WINDOW: stops a black console window from flashing up.
#[cfg_attr(not(windows), allow(dead_code))]
pub const NO_WINDOW: u32 = 0x0800_0000;

static FFMPEG: OnceLock<Result<PathBuf, String>> = OnceLock::new();

#[cfg(feature = "bundle-ffmpeg")]
static EMBEDDED: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/ffmpeg.exe"));

/// Path of a working ffmpeg.exe. The first call may take a second (it unpacks the built-in copy).
pub fn path() -> Result<PathBuf, String> {
    FFMPEG.get_or_init(locate).clone()
}

pub fn ready() -> bool {
    matches!(FFMPEG.get(), Some(Ok(_)))
}

fn locate() -> Result<PathBuf, String> {
    #[cfg(feature = "bundle-ffmpeg")]
    {
        // Unpack into %LOCALAPPDATA% (always writable), never beside the EXE.
        let dir = crate::settings::app_data_dir().join("engine");
        std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
        let name = format!("ffmpeg-{}.exe", EMBEDDED.len());
        let target = dir.join(&name);
        let ok = std::fs::metadata(&target).map(|m| m.len() == EMBEDDED.len() as u64).unwrap_or(false);
        if !ok {
            // Write to a temporary name first, then rename, so an interrupted copy is never used.
            let tmp = dir.join(format!("ffmpeg-{}.partial", std::process::id()));
            std::fs::write(&tmp, EMBEDDED).map_err(|e| format!("Cannot unpack media engine: {e}"))?;
            let _ = std::fs::remove_file(&target);
            if let Err(e) = std::fs::rename(&tmp, &target) {
                let _ = std::fs::remove_file(&tmp);
                let ok_now = std::fs::metadata(&target).map(|m| m.len() == EMBEDDED.len() as u64).unwrap_or(false);
                if !ok_now {
                    return Err(format!("Cannot unpack media engine: {e}"));
                }
            }
            // Remove copies left by older versions.
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten() {
                    let n = e.file_name().to_string_lossy().to_string();
                    if n.starts_with("ffmpeg-") && n != name {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
        }
        return Ok(target);
    }
    #[allow(unreachable_code)]
    {
        let beside = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("ffmpeg.exe")))
            .filter(|p| p.is_file());
        if let Some(p) = beside {
            return Ok(p);
        }
        let mut probe = Command::new("ffmpeg");
        hide(&mut probe);
        let works = probe
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if works {
            Ok(PathBuf::from("ffmpeg"))
        } else {
            Err("FFmpeg was not found. Build with the bundle-ffmpeg feature or put ffmpeg.exe beside the program.".into())
        }
    }
}

/// A hidden FFmpeg command.
pub fn cmd() -> Result<Command, String> {
    let mut c = Command::new(path()?);
    hide(&mut c);
    c.stdin(Stdio::null());
    Ok(c)
}

/// Stops a console window from appearing for this process.
pub fn hide(c: &mut Command) {
    #[cfg(windows)]
    c.creation_flags(NO_WINDOW);
    #[cfg(not(windows))]
    let _ = c;
}

#[derive(Clone, Default, Debug)]
pub struct MediaInfo {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub has_video: bool,
    pub has_audio: bool,
}

/// Reads duration, size and stream types from a media file.
pub fn probe(file: &Path) -> Result<MediaInfo, String> {
    let out = cmd()?
        .args(["-hide_banner", "-i"])
        .arg(file)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stderr);
    let mut info = MediaInfo::default();
    if let Some(d) = text.split("Duration: ").nth(1).and_then(|x| x.split(',').next()) {
        info.duration = parse_clock(d.trim()).unwrap_or(0.0);
    }
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with("Stream #") {
            continue;
        }
        if line.contains("Audio:") {
            info.has_audio = true;
        } else if line.contains("Video:") && !line.contains("attached pic") && !info.has_video {
            for part in line.split(',') {
                let tok = part.split_whitespace().next().unwrap_or("");
                if let Some((w, h)) = tok.split_once('x') {
                    if let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) {
                        if (16..=16384).contains(&w) && (16..=16384).contains(&h) {
                            info.width = w;
                            info.height = h;
                            info.has_video = true;
                        }
                    }
                }
                let p = part.trim();
                if let Some(f) = p.strip_suffix(" fps") {
                    info.fps = f.trim().parse().unwrap_or(0.0);
                }
            }
        }
    }
    if !info.has_audio && !info.has_video {
        return Err("This file has no readable audio or video.".into());
    }
    Ok(info)
}

/// "01:02:03.45" -> seconds.
pub fn parse_clock(s: &str) -> Option<f64> {
    let p: Vec<&str> = s.split(':').collect();
    if p.len() != 3 {
        return None;
    }
    Some(p[0].parse::<f64>().ok()? * 3600.0 + p[1].parse::<f64>().ok()? * 60.0 + p[2].parse::<f64>().ok()?)
}

/// One video frame as an image, scaled to `width` pixels wide.
pub fn frame(file: &Path, at: f64, width: u32) -> Option<image::RgbaImage> {
    let out = cmd()
        .ok()?
        .args(["-hide_banner", "-loglevel", "error", "-ss", &format!("{:.3}", at.max(0.0)), "-i"])
        .arg(file)
        .args(["-frames:v", "1", "-vf", &format!("scale={width}:-2"), "-f", "image2pipe", "-vcodec", "png", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if out.stdout.is_empty() {
        return None;
    }
    image::load_from_memory(&out.stdout).ok().map(|i| i.to_rgba8())
}

/// Left/right peak levels for drawing the waveform.
pub struct Waveform {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub duration: f64,
}

pub fn waveform(file: &Path, buckets: usize) -> Option<Waveform> {
    const RATE: usize = 1000;
    let mut child = cmd()
        .ok()?
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(file)
        .args(["-vn", "-ac", "2", "-ar", &RATE.to_string(), "-f", "f32le", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut bytes = Vec::new();
    child.stdout.take()?.read_to_end(&mut bytes).ok()?;
    let _ = child.wait();
    let frames = bytes.len() / 8;
    if frames == 0 {
        return None;
    }
    let step = (frames as f64 / buckets as f64).max(1.0);
    let n = ((frames as f64) / step).ceil() as usize;
    let (mut left, mut right) = (vec![0f32; n], vec![0f32; n]);
    for i in 0..frames {
        let o = i * 8;
        let l = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]).abs();
        let r = f32::from_le_bytes([bytes[o + 4], bytes[o + 5], bytes[o + 6], bytes[o + 7]]).abs();
        let b = ((i as f64) / step) as usize;
        if b < n {
            left[b] = left[b].max(l);
            right[b] = right[b].max(r);
        }
    }
    Some(Waveform { left, right, duration: frames as f64 / RATE as f64 })
}

/// Runs FFmpeg with a progress bar (expected = output length in seconds) and cancel support.
/// Returns the command log text.
pub fn run(args: &[String], expected: f64, ctl: &JobCtl) -> Result<String, String> {
    let mut c = cmd()?;
    c.args(["-hide_banner", "-nostdin", "-loglevel", "error", "-progress", "pipe:1", "-nostats"]);
    c.args(args);
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().map_err(|e| format!("Cannot start FFmpeg: {e}"))?;
    let stderr = child.stderr.take();
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut e) = stderr {
            let _ = e.read_to_string(&mut s);
        }
        s
    });
    let mut cancelled = false;
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if ctl.cancelled() {
                cancelled = true;
                let _ = child.kill();
                break;
            }
            if let Some(v) = line.strip_prefix("out_time_us=").or_else(|| line.strip_prefix("out_time_ms=")) {
                if let Ok(us) = v.trim().parse::<f64>() {
                    if expected > 0.0 {
                        ctl.set_progress((us / 1_000_000.0 / expected) as f32);
                    }
                }
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let errors = err_thread.join().unwrap_or_default();
    let log = format!("ffmpeg {}\n{}", args.iter().map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.clone() }).collect::<Vec<_>>().join(" "), errors);
    if cancelled || ctl.cancelled() {
        return Err("Cancelled".into());
    }
    if status.success() {
        ctl.set_progress(1.0);
        Ok(log)
    } else {
        let last: Vec<&str> = errors.lines().filter(|l| !l.trim().is_empty()).collect();
        let tail = last.iter().rev().take(3).rev().cloned().collect::<Vec<_>>().join(" | ");
        Err(if tail.is_empty() { format!("FFmpeg failed ({status})") } else { tail })
    }
}

pub fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}
