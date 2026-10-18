//! Program state and actions (recording, playback, editing, exporting).
use crate::audio::{self, DeviceList};
use crate::capture::{self, AudioMode, RecordRequest, Session};
use crate::edit::{self, AudioTrack, EditState, Project, VideoClip, FORMATS};
use crate::ffmpeg::{self, MediaInfo, Waveform};
use crate::jobs::{Done, Job};
use crate::library::Library;
use crate::player::Player;
use crate::settings::Settings;
use crate::win;
#[cfg(not(windows))]
use crate::preview_stubs::rfd;
use eframe::egui::{self, TextureHandle};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Record,
    Timeline,
    Library,
    Export,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Recordings,
    AudioEditor,
    VideoEditor,
}

#[derive(Clone, PartialEq)]
pub enum Dialog {
    None,
    Trim,
    Volume,
    Crop,
    Delete(PathBuf),
    Rename(PathBuf, String),
    Region,
    CloseConfirm,
    PreviewSettings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Ok,
    Busy,
    Warn,
    Error,
}

pub struct Screen {
    pub label: String,
    pub rect: Option<win::RECT>, // None = custom region
}

pub struct Studio {
    pub settings: Settings,
    pub page: Page,
    pub tab: Tab,
    pub mode: AudioMode,
    pub devices: DeviceList,
    pub speaker_sel: Option<usize>,
    pub mic_sel: Option<usize>,
    pub screens: Vec<Screen>,
    pub screen_sel: usize,
    pub session: Option<Session>,
    pub job: Option<Job>,
    pub library: Library,
    pub selected: Option<PathBuf>,
    pub info: Option<MediaInfo>,
    info_rx: Option<Receiver<(PathBuf, Result<MediaInfo, String>)>>,
    pub edit: EditState,
    committed: EditState,
    undo: Vec<EditState>,
    redo: Vec<EditState>,
    pub playhead: f64,
    pub player: Option<Player>,
    pub muted: bool,
    pub loop_play: bool,
    pub preview: Option<TextureHandle>,
    preview_seq: u64,
    still_rx: Option<Receiver<(PathBuf, f64, Option<image::RgbaImage>)>>,
    still_pending: Option<f64>,
    pub waveform: Option<Waveform>,
    wave_rx: Option<Receiver<(PathBuf, Option<Waveform>)>>,
    pub wave_loading: bool,
    pub crop_mode: bool,
    pub crop_drag: Option<egui::Pos2>,
    pub status: String,
    pub status_kind: StatusKind,
    pub last_log: String,
    pub export_path: String,
    pub tracks: Vec<AudioTrack>,
    pub clips: Vec<VideoClip>,
    pub clip_sel: Option<usize>,
    pub mix_format: usize,
    pub dialog: Dialog,
    pub fullscreen: bool,
    close_after_job: bool,
    allow_close: bool,
    pub search: String,
    pub lib_filter: usize,
    engine_rx: Option<Receiver<Result<PathBuf, String>>>,
    pub engine: Option<Result<PathBuf, String>>,
    pub dev_rx: Option<Receiver<DeviceList>>,
    pub autoplay: bool,
    pub seek_drag: Option<f64>,
}

impl Studio {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        crate::theme::setup(ctx);
        let settings = Settings::load();
        let _ = std::fs::create_dir_all(&settings.output_dir);
        // Prepare the built-in FFmpeg in the background (unpacks once, takes a second).
        let (etx, erx) = channel();
        let c2 = ctx.clone();
        std::thread::spawn(move || {
            let _ = etx.send(ffmpeg::path());
            c2.request_repaint();
        });
        let mut library = Library::new(ctx);
        library.refresh(&settings.output_dir);
        let mut s = Self {
            mode: AudioMode::from_u8(settings.audio_mode),
            page: Page::Record,
            tab: Tab::Recordings,
            devices: DeviceList::default(),
            speaker_sel: None,
            mic_sel: None,
            screens: Vec::new(),
            screen_sel: 0,
            session: None,
            job: None,
            library,
            selected: None,
            info: None,
            info_rx: None,
            edit: EditState::default(),
            committed: EditState::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            playhead: 0.0,
            player: None,
            muted: false,
            loop_play: false,
            preview: None,
            preview_seq: 0,
            still_rx: None,
            still_pending: None,
            waveform: None,
            wave_rx: None,
            wave_loading: false,
            crop_mode: false,
            crop_drag: None,
            status: "Ready".into(),
            status_kind: StatusKind::Ok,
            last_log: String::new(),
            export_path: String::new(),
            tracks: Vec::new(),
            clips: Vec::new(),
            clip_sel: None,
            mix_format: 0,
            dialog: Dialog::None,
            fullscreen: false,
            close_after_job: false,
            allow_close: false,
            search: String::new(),
            lib_filter: 0,
            engine_rx: Some(erx),
            engine: None,
            dev_rx: None,
            autoplay: false,
            seek_drag: None,
            settings,
        };
        s.refresh_devices(ctx);
        s.refresh_screens();
        if let Some(first) = s.library.entries.first().map(|e| e.path.clone()) {
            s.select(first, ctx);
        }
        #[cfg(feature = "preview")]
        s.preview_hook();
        s
    }

    /// Design-preview build only: open a given page for screenshots.
    #[cfg(feature = "preview")]
    fn preview_hook(&mut self) {
        match std::env::var("MCS_PAGE").unwrap_or_default().as_str() {
            "timeline" => self.page = Page::Timeline,
            "library" => self.page = Page::Library,
            "export" => self.page = Page::Export,
            "settings" => self.page = Page::Settings,
            "audio" => self.tab = Tab::AudioEditor,
            "video" => self.tab = Tab::VideoEditor,
            "trim" => self.dialog = Dialog::Trim,
            "close" => self.dialog = Dialog::CloseConfirm,
            _ => {}
        }
        if std::env::var("MCS_DEMO").is_ok() {
            self.edit.trim_start = 4.0;
            self.edit.trim_end = 15.0;
            self.edit.gain_db = 3.0;
            self.edit.normalize = true;
            self.playhead = 7.0;
            if let Some(p) = self.selected.clone() {
                let d = crate::ffmpeg::probe(&p).map(|i| i.duration).unwrap_or(20.0);
                self.clips.push(VideoClip { path: p.to_string_lossy().into(), start: 0.0, end: d, has_audio: true });
                self.tracks.push(AudioTrack { path: p.to_string_lossy().into(), start: 2.0, gain_db: 0.0, muted: false, duration: d });
            }
        }
        if std::env::var("MCS_PLAY").is_ok() {
            self.autoplay = true;
        }
    }

    // ---------- status ----------
    pub fn ok(&mut self, m: impl Into<String>) {
        self.status = m.into();
        self.status_kind = StatusKind::Ok;
    }
    pub fn warn(&mut self, m: impl Into<String>) {
        self.status = m.into();
        self.status_kind = StatusKind::Warn;
    }
    pub fn error(&mut self, m: impl Into<String>) {
        self.status = m.into();
        self.status_kind = StatusKind::Error;
    }

    // ---------- devices and screens ----------
    pub fn refresh_devices(&mut self, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let c = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(audio::list_devices());
            c.request_repaint();
        });
        self.dev_rx = Some(rx);
    }

    pub fn refresh_screens(&mut self) {
        let mons = win::monitors();
        let mut v: Vec<Screen> = Vec::new();
        let mut n = 2;
        for m in &mons {
            let label = if m.primary {
                format!("Primary Monitor ({} x {})", m.width(), m.height())
            } else {
                let l = format!("Monitor {n} ({} x {})", m.width(), m.height());
                n += 1;
                l
            };
            v.push(Screen { label, rect: Some(m.rect) });
        }
        if mons.len() > 1 || mons.is_empty() {
            let r = win::virtual_screen();
            v.push(Screen { label: format!("All Monitors ({} x {})", r.right - r.left, r.bottom - r.top), rect: Some(r) });
        }
        v.push(Screen { label: "Custom Region...".into(), rect: None });
        self.screen_sel = self.screen_sel.min(v.len() - 1);
        self.screens = v;
    }

    pub fn screen_label(&self) -> String {
        match self.screens.get(self.screen_sel) {
            Some(s) if s.rect.is_none() => {
                let [x, y, w, h] = self.settings.custom_region;
                format!("Region {w} x {h} at {x},{y}")
            }
            Some(s) => s.label.clone(),
            None => "Primary Monitor".into(),
        }
    }

    pub fn speaker_label(&self) -> String {
        match self.speaker_sel.and_then(|i| self.devices.speakers.get(i)) {
            Some(d) => d.name.clone(),
            None if self.devices.default_speaker.is_empty() => "Default speaker".into(),
            None => format!("Default speaker ({})", self.devices.default_speaker),
        }
    }
    pub fn mic_label(&self) -> String {
        match self.mic_sel.and_then(|i| self.devices.microphones.get(i)) {
            Some(d) => d.name.clone(),
            None if self.devices.default_microphone.is_empty() => "Default microphone".into(),
            None => format!("Default microphone ({})", self.devices.default_microphone),
        }
    }

    // ---------- recording ----------
    pub fn recording(&self) -> bool {
        self.session.is_some()
    }

    pub fn start_recording(&mut self) {
        if self.session.is_some() {
            return;
        }
        if self.job.is_some() {
            self.warn("Please wait for the current task to finish before recording.");
            return;
        }
        if !ffmpeg::ready() && self.settings.record_screen {
            self.warn("The media engine is still starting up - try again in a moment.");
            return;
        }
        self.stop_playback();
        let screen = if self.settings.record_screen {
            match self.screens.get(self.screen_sel).and_then(|s| s.rect) {
                Some(r) => Some(r),
                None => {
                    let [x, y, w, h] = self.settings.custom_region;
                    if w < 16 || h < 16 {
                        self.error("The custom region is too small. Set it with Custom Region... in Recording Options.");
                        return;
                    }
                    Some(win::RECT { left: x, top: y, right: x + w, bottom: y + h })
                }
            }
        } else {
            None
        };
        let req = RecordRequest {
            mode: self.mode,
            speaker_id: self.speaker_sel.and_then(|i| self.devices.speakers.get(i)).map(|d| d.id.clone()),
            mic_id: self.mic_sel.and_then(|i| self.devices.microphones.get(i)).map(|d| d.id.clone()),
            screen,
            cursor: self.settings.record_cursor,
            highlight: self.settings.highlight_clicks,
            quality: self.settings.video_quality,
            fps: self.settings.frame_rate,
            out_dir: self.settings.output_dir.clone(),
            keep_raw: self.settings.keep_raw_tracks,
        };
        match capture::start(req) {
            Ok(s) => {
                self.session = Some(s);
                self.status = "Recording... press STOP & SAVE when finished.".into();
                self.status_kind = StatusKind::Busy;
            }
            Err(e) => self.error(e),
        }
    }

    pub fn stop_recording(&mut self, ctx: &egui::Context) {
        if let Some(mut s) = self.session.take() {
            s.signal_stop();
            self.job = Some(Job::spawn("Saving recording", false, ctx, move |ctl| s.finish(ctl)));
            self.status = "Saving recording...".into();
            self.status_kind = StatusKind::Busy;
        }
    }

    pub fn record_seconds(&self) -> u64 {
        self.session.as_ref().map(|s| s.started.elapsed().as_secs()).unwrap_or(0)
    }

    // ---------- selection / preview ----------
    pub fn select(&mut self, path: PathBuf, ctx: &egui::Context) {
        if self.selected.as_ref() == Some(&path) && self.info.is_some() {
            return;
        }
        self.stop_playback();
        self.selected = Some(path.clone());
        self.info = None;
        self.preview = None;
        self.waveform = None;
        self.wave_loading = true;
        self.playhead = 0.0;
        self.crop_mode = false;
        self.edit = EditState::default();
        self.committed = self.edit.clone();
        self.undo.clear();
        self.redo.clear();
        // Details
        if let Some(info) = self.library.get(&path).and_then(|e| e.info.clone()) {
            self.on_info(path.clone(), info, ctx);
        } else {
            let (tx, rx) = channel();
            let (p, c) = (path.clone(), ctx.clone());
            std::thread::spawn(move || {
                let _ = tx.send((p.clone(), ffmpeg::probe(&p)));
                c.request_repaint();
            });
            self.info_rx = Some(rx);
        }
        // Waveform
        let (tx, rx) = channel();
        let (p, c) = (path.clone(), ctx.clone());
        std::thread::spawn(move || {
            let w = ffmpeg::waveform(&p, 1600);
            let _ = tx.send((p, w));
            c.request_repaint();
        });
        self.wave_rx = Some(rx);
        self.ok(format!("Selected: {}", edit::name(&path)));
    }

    fn on_info(&mut self, path: PathBuf, info: MediaInfo, ctx: &egui::Context) {
        if self.selected.as_ref() != Some(&path) {
            return;
        }
        let video = info.has_video;
        self.info = Some(info);
        if video {
            self.request_still(self.playhead, ctx);
        }
        if self.autoplay {
            self.autoplay = false;
            self.play(ctx);
        }
    }

    pub fn request_still(&mut self, t: f64, ctx: &egui::Context) {
        let Some(path) = self.selected.clone() else { return };
        if !self.info.as_ref().map(|i| i.has_video).unwrap_or(false) {
            return;
        }
        if self.still_rx.is_some() {
            self.still_pending = Some(t);
            return;
        }
        let (tx, rx) = channel();
        let c = ctx.clone();
        std::thread::spawn(move || {
            let img = ffmpeg::frame(&path, t, 960);
            let _ = tx.send((path, t, img));
            c.request_repaint();
        });
        self.still_rx = Some(rx);
    }

    fn set_preview(&mut self, ctx: &egui::Context, w: usize, h: usize, rgba: &[u8]) {
        let img = egui::ColorImage::from_rgba_unmultiplied([w, h], rgba);
        match &mut self.preview {
            Some(t) if t.size() == [w, h] => t.set(img, egui::TextureOptions::LINEAR),
            _ => self.preview = Some(ctx.load_texture("preview", img, egui::TextureOptions::LINEAR)),
        }
    }

    // ---------- playback ----------
    pub fn playing(&self) -> bool {
        self.player.is_some()
    }

    pub fn play(&mut self, ctx: &egui::Context) {
        let (Some(path), Some(info)) = (self.selected.clone(), self.info.clone()) else {
            self.warn("Select a recording first.");
            return;
        };
        if info.duration > 0.0 && self.playhead >= info.duration - 0.05 {
            self.playhead = 0.0;
        }
        let vol = if self.muted { 0.0 } else { self.settings.preview_volume };
        match Player::open(&path, &info, self.playhead, vol, 960, ctx) {
            Ok(p) => self.player = Some(p),
            Err(e) => self.error(format!("Cannot play this file: {e}")),
        }
    }

    pub fn stop_playback(&mut self) {
        if let Some(p) = self.player.take() {
            self.playhead = p.position();
            drop(p);
        }
    }

    pub fn toggle_play(&mut self, ctx: &egui::Context) {
        if self.playing() {
            self.stop_playback();
        } else {
            self.play(ctx);
        }
    }

    pub fn seek(&mut self, t: f64, ctx: &egui::Context) {
        let dur = self.duration();
        self.playhead = if dur > 0.0 { t.clamp(0.0, dur) } else { t.max(0.0) };
        if self.player.is_some() {
            self.player = None;
            self.play(ctx);
        } else {
            self.request_still(self.playhead, ctx);
        }
    }

    pub fn set_volume(&mut self, v: f32) {
        self.settings.preview_volume = v.clamp(0.0, 1.0);
        self.muted = false;
        if let Some(p) = &self.player {
            p.set_volume(self.settings.preview_volume);
        }
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        if let Some(p) = &self.player {
            p.set_volume(if self.muted { 0.0 } else { self.settings.preview_volume });
        }
    }

    pub fn duration(&self) -> f64 {
        self.info.as_ref().map(|i| i.duration).filter(|d| *d > 0.0).or(self.waveform.as_ref().map(|w| w.duration)).unwrap_or(0.0)
    }

    // ---------- undo ----------
    fn track_undo(&mut self, ctx: &egui::Context) {
        if self.edit != self.committed && !ctx.input(|i| i.pointer.any_down()) {
            self.undo.push(self.committed.clone());
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.committed = self.edit.clone();
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.edit != self.committed
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo(&mut self) {
        if self.edit != self.committed {
            self.undo.push(self.committed.clone());
            self.committed = self.edit.clone();
        }
        if let Some(s) = self.undo.pop() {
            self.redo.push(self.edit.clone());
            self.edit = s.clone();
            self.committed = s;
            self.ok("Undo");
        }
    }
    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(self.edit.clone());
            self.edit = s.clone();
            self.committed = s;
            self.ok("Redo");
        }
    }
    pub fn reset_edits(&mut self) {
        self.edit = EditState::default();
        self.crop_mode = false;
        self.ok("All edits cleared (Undo brings them back).");
    }

    // ---------- jobs ----------
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn start_job<F>(&mut self, label: &str, ctx: &egui::Context, f: F)
    where
        F: FnOnce(&crate::jobs::JobCtl) -> Result<Done, String> + Send + 'static,
    {
        if self.job.is_some() {
            self.warn("Another task is still running. Please wait or cancel it.");
            return;
        }
        if self.session.is_some() {
            self.warn("Stop the recording first.");
            return;
        }
        self.job = Some(Job::spawn(label, true, ctx, f));
        self.status = format!("{label}...");
        self.status_kind = StatusKind::Busy;
    }

    fn output_for(&self, input: &Path, tag: &str, ext: &str) -> PathBuf {
        let stem = input.file_stem().unwrap_or_default().to_string_lossy().to_string();
        edit::unique(&self.settings.output_dir, &stem, tag, ext)
    }

    /// "Export Selected": applies every edit with the chosen format and quality.
    pub fn export_selected(&mut self, ctx: &egui::Context) {
        let (Some(input), Some(info)) = (self.selected.clone(), self.info.clone()) else {
            self.warn("Select a file to export first.");
            return;
        };
        let fmt = &FORMATS[self.settings.export_format.min(FORMATS.len() - 1)];
        let output = if self.export_path.trim().is_empty() {
            self.output_for(&input, "Edited", fmt.ext)
        } else {
            PathBuf::from(self.export_path.trim()).with_extension(fmt.ext)
        };
        let (args, len) = match edit::export_args(&input, &info, &self.edit, fmt, self.settings.export_quality, &output) {
            Ok(v) => v,
            Err(e) => {
                self.error(e);
                return;
            }
        };
        self.start_job("Exporting", ctx, move |ctl| {
            let log = ffmpeg::run(&args, len, ctl)?;
            Ok(Done { message: format!("Exported: {}", edit::name(&output)), select: Some(output), log })
        });
    }

    /// Quick-tool "save a copy" with only some edits applied, in the original format.
    pub fn save_copy(&mut self, ctx: &egui::Context, tag: &str, e: EditState) {
        let (Some(input), Some(info)) = (self.selected.clone(), self.info.clone()) else {
            self.warn("Select a file first.");
            return;
        };
        let src_ext = input.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        let idx = FORMATS
            .iter()
            .position(|f| f.ext == src_ext && (f.video == info.has_video || !f.video))
            .unwrap_or(if info.has_video { 0 } else { 5 });
        let fmt = &FORMATS[idx];
        let output = self.output_for(&input, tag, fmt.ext);
        let (args, len) = match edit::export_args(&input, &info, &e, fmt, 1, &output) {
            Ok(v) => v,
            Err(err) => {
                self.error(err);
                return;
            }
        };
        let label = format!("Saving {tag} copy");
        self.start_job(&label, ctx, move |ctl| {
            let log = ffmpeg::run(&args, len, ctl)?;
            Ok(Done { message: format!("Saved: {}", edit::name(&output)), select: Some(output), log })
        });
    }

    pub fn split(&mut self, ctx: &egui::Context) {
        let (Some(input), Some(info)) = (self.selected.clone(), self.info.clone()) else {
            self.warn("Select a file to split first.");
            return;
        };
        self.stop_playback();
        let at = self.playhead;
        let dir = self.settings.output_dir.clone();
        self.start_job("Splitting", ctx, move |ctl| edit::split(&input, &info, at, &dir, ctl));
    }

    pub fn render_timeline(&mut self, ctx: &egui::Context) {
        let clips = self.clips.clone();
        let out = self.settings.output_dir.join(format!("Timeline {}.mp4", win::local_stamp()));
        self.start_job("Rendering timeline", ctx, move |ctl| edit::render_timeline(&clips, &out, ctl));
    }

    pub fn export_mix(&mut self, ctx: &egui::Context) {
        let tracks = self.tracks.clone();
        let ext = ["wav", "mp3", "flac", "m4a"][self.mix_format.min(3)];
        let out = self.settings.output_dir.join(format!("Mix {}.{ext}", win::local_stamp()));
        self.start_job("Mixing tracks", ctx, move |ctl| edit::mix_tracks(&tracks, &out, ctl));
    }

    pub fn add_clip(&mut self) {
        let (Some(p), Some(info)) = (self.selected.clone(), self.info.clone()) else {
            self.warn("Select a video file first.");
            return;
        };
        if !info.has_video {
            self.warn("That file has no video. Add it to the audio mix instead.");
            return;
        }
        let (start, end) = self.edit.selection(info.duration);
        self.clips.push(VideoClip { path: p.to_string_lossy().into(), start, end, has_audio: info.has_audio });
        self.clip_sel = Some(self.clips.len() - 1);
        self.ok("Clip added to the video timeline (uses the current trim In/Out).");
    }

    pub fn add_track(&mut self) {
        let (Some(p), Some(info)) = (self.selected.clone(), self.info.clone()) else {
            self.warn("Select a file first.");
            return;
        };
        if !info.has_audio {
            self.warn("That file has no audio.");
            return;
        }
        self.tracks.push(AudioTrack { path: p.to_string_lossy().into(), start: 0.0, gain_db: 0.0, muted: false, duration: info.duration });
        self.ok("Track added to the audio mix.");
    }

    pub fn delete_file(&mut self, path: &Path, ctx: &egui::Context) {
        if self.selected.as_deref() == Some(path) {
            self.stop_playback();
            self.selected = None;
            self.info = None;
            self.preview = None;
            self.waveform = None;
        }
        match win::recycle(path) {
            Ok(()) => self.ok(format!("Moved to Recycle Bin: {}", edit::name(path))),
            Err(e) => self.error(e),
        }
        self.library.refresh(&self.settings.output_dir);
        if self.selected.is_none() {
            if let Some(first) = self.library.entries.first().map(|e| e.path.clone()) {
                self.select(first, ctx);
            }
        }
    }

    pub fn rename_file(&mut self, path: &Path, new_name: &str, ctx: &egui::Context) {
        let clean: String = new_name.chars().filter(|c| !"\\/:*?\"<>|".contains(*c)).collect();
        let clean = clean.trim();
        if clean.is_empty() {
            self.warn("Enter a file name.");
            return;
        }
        let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");
        let target = path.with_file_name(format!("{clean}.{ext}"));
        if target.exists() {
            self.warn("A file with that name already exists.");
            return;
        }
        let was_selected = self.selected.as_deref() == Some(path);
        if was_selected {
            self.stop_playback();
        }
        match std::fs::rename(path, &target) {
            Ok(()) => {
                self.ok(format!("Renamed to {}", edit::name(&target)));
                self.library.refresh(&self.settings.output_dir);
                if was_selected {
                    self.selected = None;
                    self.select(target, ctx);
                }
            }
            Err(e) => self.error(format!("Rename failed: {e}")),
        }
    }

    pub fn import_files(&mut self, ctx: &egui::Context) {
        let Some(files) = rfd::FileDialog::new()
            .set_title("Add media files to the library")
            .add_filter("Media files", &crate::library::MEDIA_EXT)
            .pick_files()
        else {
            return;
        };
        let mut n = 0;
        let mut last = None;
        for f in files {
            let target = self.settings.output_dir.join(f.file_name().unwrap_or_default());
            if target == f {
                continue;
            }
            if !target.exists() && std::fs::copy(&f, &target).is_ok() {
                n += 1;
                last = Some(target);
            }
        }
        self.library.refresh(&self.settings.output_dir);
        if let Some(l) = last {
            self.select(l, ctx);
        }
        self.ok(format!("Added {n} file(s) to the library."));
    }

    pub fn open_external_file(&mut self, ctx: &egui::Context) {
        if let Some(f) = rfd::FileDialog::new().add_filter("Media files", &crate::library::MEDIA_EXT).pick_file() {
            self.selected = None;
            self.select(f, ctx);
        }
    }

    pub fn change_output_folder(&mut self, ctx: &egui::Context) {
        if let Some(d) = rfd::FileDialog::new().set_title("Choose the output folder").set_directory(&self.settings.output_dir).pick_folder() {
            self.settings.output_dir = d;
            self.settings.save();
            self.library.refresh(&self.settings.output_dir);
            self.ok(format!("Output folder: {}", self.settings.output_dir.display()));
            let _ = ctx;
        }
    }

    pub fn open_folder(&self, p: &Path) {
        let _ = std::fs::create_dir_all(p);
        let _ = std::process::Command::new("explorer.exe").arg(p).spawn();
    }

    pub fn show_in_explorer(&self, p: &Path) {
        let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", p.display())).spawn();
    }

    pub fn save_project(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Morrison Studio Project", &["json"]).set_file_name("Project.json").set_directory(&self.settings.output_dir).save_file() else {
            return;
        };
        let proj = Project {
            media: self.selected.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            edit: self.edit.clone(),
            tracks: self.tracks.clone(),
            clips: self.clips.clone(),
        };
        match serde_json::to_vec_pretty(&proj).map_err(|e| e.to_string()).and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string())) {
            Ok(()) => self.ok(format!("Project saved: {}", edit::name(&path))),
            Err(e) => self.error(format!("Project save failed: {e}")),
        }
    }

    pub fn open_project(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new().add_filter("Morrison Studio Project", &["json"]).set_directory(&self.settings.output_dir).pick_file() else {
            return;
        };
        match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<Project>(&b).map_err(|e| e.to_string())) {
            Ok(p) => {
                self.tracks = p.tracks;
                self.clips = p.clips;
                if !p.media.is_empty() && Path::new(&p.media).is_file() {
                    self.selected = None;
                    self.select(PathBuf::from(&p.media), ctx);
                }
                self.edit = p.edit;
                self.committed = self.edit.clone();
                self.ok(format!("Project opened: {}", edit::name(&path)));
            }
            Err(e) => self.error(format!("Could not open project: {e}")),
        }
    }

    // ---------- per-frame housekeeping ----------
    pub fn tick(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.engine_rx {
            if let Ok(r) = rx.try_recv() {
                if let Err(e) = &r {
                    let e = e.clone();
                    self.error(e);
                }
                self.engine = Some(r);
                self.engine_rx = None;
            }
        }
        if let Some(rx) = &self.dev_rx {
            if let Ok(d) = rx.try_recv() {
                self.devices = d;
                if self.speaker_sel.map(|i| i >= self.devices.speakers.len()).unwrap_or(false) {
                    self.speaker_sel = None;
                }
                if self.mic_sel.map(|i| i >= self.devices.microphones.len()).unwrap_or(false) {
                    self.mic_sel = None;
                }
                self.dev_rx = None;
            }
        }
        self.library.update(ctx);
        if let Some(rx) = &self.info_rx {
            if let Ok((p, r)) = rx.try_recv() {
                self.info_rx = None;
                match r {
                    Ok(info) => self.on_info(p, info, ctx),
                    Err(e) => {
                        if self.selected.as_ref() == Some(&p) {
                            self.error(e);
                        }
                    }
                }
            }
        }
        if let Some(rx) = &self.wave_rx {
            if let Ok((p, w)) = rx.try_recv() {
                self.wave_rx = None;
                if self.selected.as_ref() == Some(&p) {
                    self.waveform = w;
                    self.wave_loading = false;
                }
            }
        }
        if let Some(rx) = &self.still_rx {
            if let Ok((p, _t, img)) = rx.try_recv() {
                self.still_rx = None;
                if self.selected.as_ref() == Some(&p) && self.player.is_none() {
                    if let Some(img) = img {
                        let (w, h) = (img.width() as usize, img.height() as usize);
                        self.set_preview(ctx, w, h, img.as_raw());
                    }
                }
                if let Some(t) = self.still_pending.take() {
                    self.request_still(t, ctx);
                }
            }
        }
        // Playback
        let mut finished = false;
        let mut new_frame = None;
        if let Some(p) = &self.player {
            self.playhead = self.seek_drag.unwrap_or_else(|| p.position());
            if let Ok(mut f) = p.frame.lock() {
                if let Some(fr) = f.as_ref() {
                    if fr.seq != self.preview_seq {
                        new_frame = f.take();
                    }
                }
            }
            finished = p.finished();
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        if let Some(fr) = new_frame {
            self.preview_seq = fr.seq;
            self.set_preview(ctx, fr.width, fr.height, &fr.rgba);
        }
        if finished {
            self.player = None;
            if self.loop_play {
                self.playhead = self.edit.trim_start;
                self.play(ctx);
            } else {
                self.playhead = self.duration();
            }
        }
        // Recording watchdog
        if let Some(s) = self.session.as_mut() {
            if let Some(err) = s.video_failed() {
                self.stop_recording(ctx);
                self.error(err);
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        // Jobs
        if let Some(job) = &self.job {
            if let Some(result) = job.poll() {
                self.job = None;
                match result {
                    Ok(done) => {
                        self.ok(done.message);
                        if !done.log.is_empty() {
                            self.last_log = done.log;
                        }
                        self.library.refresh(&self.settings.output_dir);
                        if let Some(sel) = done.select {
                            if sel.parent() == Some(self.settings.output_dir.as_path()) || sel.is_file() {
                                self.selected = None;
                                self.select(sel, ctx);
                            }
                        }
                    }
                    Err(e) if e == "Cancelled" => self.warn("Cancelled."),
                    Err(e) => {
                        self.last_log = e.clone();
                        self.error(e);
                    }
                }
                if self.close_after_job {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        self.track_undo(ctx);
    }

    /// Stops the window closing while recording/saving, and asks what to do.
    pub fn handle_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && (self.session.is_some() || self.job.is_some()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dialog = Dialog::CloseConfirm;
        }
    }

    pub fn stop_save_and_close(&mut self, ctx: &egui::Context) {
        if self.session.is_some() {
            self.stop_recording(ctx);
        }
        self.close_after_job = true;
        if self.job.is_none() {
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub fn close_now(&mut self, ctx: &egui::Context) {
        if let Some(j) = &self.job {
            if j.cancellable {
                j.cancel();
            }
        }
        self.allow_close = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    pub fn shutdown(&mut self) {
        self.settings.audio_mode = self.mode.to_u8();
        self.settings.save();
        self.player = None;
        // If the window was closed mid-recording anyway, still finish the files properly.
        if let Some(mut s) = self.session.take() {
            s.signal_stop();
            let ctx = egui::Context::default();
            let job = Job::spawn("Saving recording", false, &ctx, move |ctl| s.finish(ctl));
            let t0 = std::time::Instant::now();
            while job.poll().is_none() && t0.elapsed().as_secs() < 60 {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }
}
