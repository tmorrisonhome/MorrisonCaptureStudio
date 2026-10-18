//! The list of recorded/edited files, with durations and thumbnails loaded in the background.
use crate::ffmpeg::{self, MediaInfo};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::SystemTime;

pub const MEDIA_EXT: [&str; 11] = ["mp4", "mkv", "webm", "mov", "avi", "gif", "wav", "mp3", "flac", "m4a", "ogg"];
pub const VIDEO_EXT: [&str; 6] = ["mp4", "mkv", "webm", "mov", "avi", "gif"];

pub fn is_media(p: &Path) -> bool {
    p.extension().and_then(|x| x.to_str()).map(|x| MEDIA_EXT.contains(&x.to_ascii_lowercase().as_str())).unwrap_or(false)
}
pub fn is_video_ext(p: &Path) -> bool {
    p.extension().and_then(|x| x.to_str()).map(|x| VIDEO_EXT.contains(&x.to_ascii_lowercase().as_str())).unwrap_or(false)
}

pub struct Entry {
    pub path: PathBuf,
    pub title: String,
    pub subtitle: String,
    pub size: u64,
    pub modified: SystemTime,
    pub info: Option<MediaInfo>,
    pub thumb: Option<eframe::egui::TextureHandle>,
    pub requested: bool,
}

struct Meta {
    path: PathBuf,
    modified: SystemTime,
    info: Option<MediaInfo>,
    thumb: Option<image::RgbaImage>,
}

pub struct Library {
    pub entries: Vec<Entry>,
    cache: HashMap<PathBuf, (SystemTime, Option<MediaInfo>, Option<eframe::egui::TextureHandle>)>,
    tx: Sender<(PathBuf, SystemTime)>,
    rx: Receiver<Meta>,
}

/// Splits "Screen Recording 2026-10-17 14-30-25.mp4" into ("Screen Recording", "2026-10-17 14-30-25.mp4").
fn split_name(p: &Path) -> (String, String) {
    let file = p.file_name().unwrap_or_default().to_string_lossy().to_string();
    let stem = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let bytes: Vec<char> = stem.chars().collect();
    for i in 0..bytes.len() {
        // Look for "YYYY-MM-DD"
        if i + 10 <= bytes.len() && i > 0 {
            let w: String = bytes[i..i + 10].iter().collect();
            let b = w.as_bytes();
            if b[4] == b'-' && b[7] == b'-' && w.chars().enumerate().all(|(k, c)| k == 4 || k == 7 || c.is_ascii_digit()) {
                let title: String = bytes[..i].iter().collect::<String>().trim().to_string();
                let rest: String = file.chars().skip(i).collect();
                if !title.is_empty() {
                    return (title, rest);
                }
            }
        }
    }
    (stem, file)
}

impl Library {
    pub fn new(ctx: &eframe::egui::Context) -> Self {
        let (tx, worker_rx) = channel::<(PathBuf, SystemTime)>();
        let (worker_tx, rx) = channel::<Meta>();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            while let Ok((path, modified)) = worker_rx.recv() {
                let info = ffmpeg::probe(&path).ok();
                let thumb = info.as_ref().filter(|i| i.has_video).and_then(|i| ffmpeg::frame(&path, (i.duration * 0.1).min(1.0), 128));
                if worker_tx.send(Meta { path, modified, info, thumb }).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self { entries: Vec::new(), cache: HashMap::new(), tx, rx }
    }

    pub fn refresh(&mut self, dir: &Path) {
        let mut list: Vec<Entry> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && is_media(p))
            .filter_map(|p| {
                let m = std::fs::metadata(&p).ok()?;
                let modified = m.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                let (title, subtitle) = split_name(&p);
                let (info, thumb) = match self.cache.get(&p) {
                    Some((t, i, th)) if *t == modified => (i.clone(), th.clone()),
                    _ => (None, None),
                };
                let requested = info.is_some();
                Some(Entry { path: p, title, subtitle, size: m.len(), modified, info, thumb, requested })
            })
            .collect();
        list.sort_by(|a, b| b.modified.cmp(&a.modified));
        self.entries = list;
    }

    /// Call every frame: queues missing details and collects finished ones.
    pub fn update(&mut self, ctx: &eframe::egui::Context) {
        for e in self.entries.iter_mut().filter(|e| !e.requested) {
            e.requested = true;
            let _ = self.tx.send((e.path.clone(), e.modified));
        }
        while let Ok(m) = self.rx.try_recv() {
            let tex = m.thumb.map(|img| {
                let size = [img.width() as usize, img.height() as usize];
                ctx.load_texture(format!("thumb:{}", m.path.display()), eframe::egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), Default::default())
            });
            self.cache.insert(m.path.clone(), (m.modified, m.info.clone(), tex.clone()));
            if let Some(e) = self.entries.iter_mut().find(|e| e.path == m.path) {
                e.info = m.info;
                e.thumb = tex;
            }
        }
    }

    pub fn get(&self, p: &Path) -> Option<&Entry> {
        self.entries.iter().find(|e| e.path == p)
    }
}

pub fn size_text(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} GB", b / 1024.0 / 1024.0 / 1024.0)
    } else if b >= 1024.0 * 1024.0 {
        format!("{:.0} MB", b / 1024.0 / 1024.0)
    } else {
        format!("{:.0} KB", (b / 1024.0).max(1.0))
    }
}

pub fn hms(t: f64) -> String {
    let t = t.max(0.0) as u64;
    format!("{:02}:{:02}:{:02}", t / 3600, (t / 60) % 60, t % 60)
}
