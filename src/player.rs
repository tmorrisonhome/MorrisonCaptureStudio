//! In-app media player. FFmpeg decodes in the background (hidden, no window); sound plays through
//! WASAPI and video frames are drawn inside the program, synced to the audio clock.
use crate::audio::{self, OutputState, Ring, RATE};
use crate::ffmpeg::{self, MediaInfo};
use std::io::Read;
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const VIDEO_FPS: f64 = 30.0;

pub struct Frame {
    pub seq: u64,
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

struct Clock {
    use_audio: bool,
    out: Arc<OutputState>,
    wall: OnceLock<Instant>,
}
impl Clock {
    fn now(&self) -> f64 {
        if self.use_audio && self.out.ok.load(Ordering::Relaxed) {
            self.out.played_frames.load(Ordering::Relaxed) as f64 / RATE as f64
        } else {
            self.wall.get().map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0)
        }
    }
}

pub struct Player {
    start: f64,
    duration: f64,
    stop: Arc<AtomicBool>,
    children: Arc<Mutex<Vec<Child>>>,
    threads: Vec<JoinHandle<()>>,
    clock: Arc<Clock>,
    out: Arc<OutputState>,
    video_done: Arc<AtomicBool>,
    has_audio: bool,
    has_video: bool,
    pub frame: Arc<Mutex<Option<Frame>>>,
}

impl Player {
    /// Starts playing `file` from `start` seconds. `video_width` = preview decode width.
    pub fn open(file: &Path, info: &MediaInfo, start: f64, volume: f32, video_width: u32, ctx: &eframe::egui::Context) -> Result<Player, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let children = Arc::new(Mutex::new(Vec::new()));
        let out = Arc::new(OutputState {
            played_frames: AtomicU64::new(0),
            ok: AtomicBool::new(false),
            started: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            volume: AtomicU32::new(volume.to_bits()),
        });
        let has_audio = info.has_audio;
        let has_video = info.has_video;
        let clock = Arc::new(Clock { use_audio: has_audio, out: out.clone(), wall: OnceLock::new() });
        let frame = Arc::new(Mutex::new(None));
        let video_done = Arc::new(AtomicBool::new(!has_video));
        let mut threads = Vec::new();
        let ss = format!("{:.3}", start.max(0.0));

        if has_audio {
            let mut child = ffmpeg::cmd()?
                .args(["-hide_banner", "-loglevel", "error", "-ss", &ss, "-i"])
                .arg(file)
                .args(["-vn", "-ac", "2", "-ar", &RATE.to_string(), "-f", "f32le", "-"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?;
            let mut pipe = child.stdout.take().ok_or("no audio pipe")?;
            children.lock().unwrap().push(child);
            let ring = Arc::new(Ring { samples: Mutex::new(Default::default()), decoder_done: AtomicBool::new(false) });
            let (r, st) = (ring.clone(), stop.clone());
            threads.push(std::thread::spawn(move || {
                let mut buf = vec![0u8; 16384];
                let mut carry: Vec<u8> = Vec::new();
                while !st.load(Ordering::Relaxed) {
                    // Keep about two seconds decoded ahead.
                    if r.samples.lock().map(|s| s.len()).unwrap_or(0) > RATE as usize * 4 {
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    match pipe.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            carry.extend_from_slice(&buf[..n]);
                            let whole = carry.len() / 4 * 4;
                            if let Ok(mut s) = r.samples.lock() {
                                for c in carry[..whole].chunks_exact(4) {
                                    s.push_back(f32::from_le_bytes([c[0], c[1], c[2], c[3]]));
                                }
                            }
                            carry.drain(..whole);
                        }
                    }
                }
                r.decoder_done.store(true, Ordering::Relaxed);
            }));
            threads.push(audio::spawn_output(ring, out.clone(), stop.clone()));
        }

        if has_video && info.width > 0 && info.height > 0 {
            let w = (video_width.max(64) / 2 * 2) as usize;
            let h = (((w as f64 * info.height as f64 / info.width as f64) / 2.0).round() * 2.0).max(2.0) as usize;
            let mut child = ffmpeg::cmd()?
                .args(["-hide_banner", "-loglevel", "error", "-ss", &ss, "-i"])
                .arg(file)
                .args(["-an", "-vf", &format!("fps={VIDEO_FPS},scale={w}:{h}:flags=bilinear"), "-pix_fmt", "rgba", "-f", "rawvideo", "-"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?;
            let mut pipe = child.stdout.take().ok_or("no video pipe")?;
            children.lock().unwrap().push(child);
            let (st, clk, fr, done, ctx) = (stop.clone(), clock.clone(), frame.clone(), video_done.clone(), ctx.clone());
            threads.push(std::thread::spawn(move || {
                let size = w * h * 4;
                let mut buf = vec![0u8; size];
                let mut idx: u64 = 0;
                while !st.load(Ordering::Relaxed) {
                    if pipe.read_exact(&mut buf).is_err() {
                        break;
                    }
                    let _ = clk.wall.set(Instant::now()); // video-only files run on the wall clock
                    let t = idx as f64 / VIDEO_FPS;
                    idx += 1;
                    // Wait until this frame is due.
                    loop {
                        if st.load(Ordering::Relaxed) {
                            return;
                        }
                        let now = clk.now();
                        if now + 0.004 >= t {
                            break;
                        }
                        std::thread::sleep(Duration::from_secs_f64((t - now).clamp(0.001, 0.010)));
                    }
                    // Drop frames we are too late for (keeps sound and picture together).
                    if clk.now() - t > 0.12 && idx > 1 {
                        continue;
                    }
                    if let Ok(mut f) = fr.lock() {
                        *f = Some(Frame { seq: idx, width: w, height: h, rgba: buf.clone() });
                    }
                    ctx.request_repaint();
                }
                done.store(true, Ordering::Relaxed);
            }));
        }

        Ok(Player { start, duration: info.duration, stop, children, threads, clock, out, video_done, has_audio, has_video, frame })
    }

    pub fn position(&self) -> f64 {
        let p = self.start + self.clock.now();
        if self.duration > 0.0 {
            p.min(self.duration)
        } else {
            p
        }
    }

    pub fn set_volume(&self, v: f32) {
        self.out.volume.store(v.to_bits(), Ordering::Relaxed);
    }

    pub fn finished(&self) -> bool {
        let audio_done = !self.has_audio || self.out.finished.load(Ordering::Relaxed);
        let video_done = !self.has_video || self.video_done.load(Ordering::Relaxed);
        (audio_done && video_done) || (self.duration > 0.0 && self.position() >= self.duration - 0.01 && audio_done)
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(mut c) = self.children.lock() {
            for ch in c.iter_mut() {
                let _ = ch.kill();
                let _ = ch.wait();
            }
            c.clear();
        }
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}
