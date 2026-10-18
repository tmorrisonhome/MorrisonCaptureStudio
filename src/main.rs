// Morrison Capture Studio - portable Windows audio/screen recorder and editor.
// Release builds open no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(all(not(windows), not(feature = "preview")))]
compile_error!("Morrison Capture Studio is a Windows program (it uses Windows audio and screen capture).");

mod app;
#[cfg(windows)]
mod audio;
#[cfg(not(windows))]
#[path = "audio_stub.rs"]
mod audio;
#[cfg(not(windows))]
mod preview_stubs;
mod capture;
mod edit;
mod ffmpeg;
mod icons;
mod jobs;
mod library;
mod pages;
mod player;
mod settings;
mod theme;
mod ui;
#[cfg(windows)]
mod win;
#[cfg(not(windows))]
#[path = "win_stub.rs"]
mod win;

use eframe::egui;

impl eframe::App for app::Studio {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_close(ctx);
        self.tick(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::draw(self, ui);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shutdown();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::BG.to_normalized_gamma_f32()
    }
}

/// Design-preview build only: runs every export/split/timeline/mix command on real files.
#[cfg(feature = "preview")]
fn selftest(dir: &str) {
    use std::path::{Path, PathBuf};
    let ctx = egui::Context::default();
    let wait = |job: jobs::Job| loop {
        if let Some(r) = job.poll() {
            break r;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let out = PathBuf::from(dir).join("selftest");
    let _ = std::fs::create_dir_all(&out);
    let video = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).find(|p| p.to_string_lossy().contains("14-30-25")).unwrap();
    let audio = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).find(|p| p.to_string_lossy().contains("Speakers Audio")).unwrap();
    let silent = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).find(|p| p.to_string_lossy().contains("14-05-18")).unwrap();
    let mut e = edit::EditState::default();
    e.trim_start = 2.0; e.trim_end = 9.0; e.gain_db = 3.0; e.speed = 1.5; e.fade_in = 1.0; e.fade_out = 1.0;
    e.normalize = true; e.bass_db = 4.0; e.treble_db = -2.0; e.noise_reduce = true; e.compressor = true; e.mono = false;
    e.crop = [100, 50, 800, 450]; e.rotation = 90; e.flip_h = true; e.brightness = 0.05; e.contrast = 1.1; e.resize = [640, 360];
    for src in [&video, &audio] {
        let info = ffmpeg::probe(src).unwrap();
        for (fi, f) in edit::FORMATS.iter().enumerate() {
            let o = out.join(format!("{}-{fi}.{}", src.file_stem().unwrap().to_string_lossy().split(' ').next().unwrap(), f.ext));
            match edit::export_args(src, &info, &e, f, 0, &o) {
                Ok((args, len)) => {
                    let a2 = args.clone();
                    let r = wait(jobs::Job::spawn("t", true, &ctx, move |c| ffmpeg::run(&a2, len, c).map(|l| jobs::Done { message: String::new(), select: None, log: l })));
                    let got = ffmpeg::probe(&o).map(|i| format!("{:.2}s v={} a={} {}x{}", i.duration, i.has_video, i.has_audio, i.width, i.height));
                    println!("EXPORT {} -> {:<6} {} | {:?}", Path::new(src).extension().unwrap().to_string_lossy(), f.ext, if r.is_ok() { "OK " } else { "FAIL" }, r.err().or(got.err()).unwrap_or_else(|| ffmpeg::probe(&o).map(|i| format!("{:.2}s v={} a={} {}x{}", i.duration, i.has_video, i.has_audio, i.width, i.height)).unwrap()));
                }
                Err(err) => println!("EXPORT {} -> {:<6} SKIP ({err})", Path::new(src).extension().unwrap().to_string_lossy(), f.ext),
            }
        }
    }
    for src in [&video, &audio] {
        let info = ffmpeg::probe(src).unwrap();
        let (s2, o2) = (src.clone(), out.clone());
        let r = wait(jobs::Job::spawn("s", true, &ctx, move |c| edit::split(&s2, &info, 5.0, &o2, c)));
        println!("SPLIT {:?}", r.map(|d| d.message));
    }
    let clips = vec![
        edit::VideoClip { path: video.to_string_lossy().into(), start: 1.0, end: 4.0, has_audio: true },
        edit::VideoClip { path: silent.to_string_lossy().into(), start: 0.0, end: 3.0, has_audio: false },
    ];
    let o = out.join("timeline.mp4");
    let o2 = o.clone();
    let r = wait(jobs::Job::spawn("tl", true, &ctx, move |c| edit::render_timeline(&clips, &o2, c)));
    println!("TIMELINE {:?} {:?}", r.map(|d| d.message), ffmpeg::probe(&o).map(|i| (i.duration, i.has_audio, i.width)));
    let tracks = vec![
        edit::AudioTrack { path: audio.to_string_lossy().into(), start: 0.0, gain_db: -3.0, muted: false, duration: 15.0 },
        edit::AudioTrack { path: video.to_string_lossy().into(), start: 2.5, gain_db: 0.0, muted: false, duration: 20.0 },
    ];
    for ext in ["wav", "mp3", "flac", "m4a"] {
        let o = out.join(format!("mix.{ext}"));
        let (t2, o2) = (tracks.clone(), o.clone());
        let r = wait(jobs::Job::spawn("mx", true, &ctx, move |c| edit::mix_tracks(&t2, &o2, c)));
        println!("MIX {ext} {:?} {:?}", r.map(|d| d.message), ffmpeg::probe(&o).map(|i| i.duration));
    }
    // waveform
    println!("WAVE {:?}", ffmpeg::waveform(&audio, 1600).map(|w| (w.left.len(), w.duration)));
}

fn main() -> eframe::Result<()> {
    #[cfg(feature = "preview")]
    if let Some(d) = std::env::args().nth(2).filter(|_| std::env::args().nth(1).as_deref() == Some("--selftest")) {
        selftest(&d);
        return Ok(());
    }
    // Real pixel coordinates on scaled displays (needed for screen regions and click highlights).
    win::make_dpi_aware();
    let size = 64;
    let icon = egui::IconData { rgba: icons::window_icon(size), width: size, height: size };
    let title = format!("Morrison Capture Studio v{}", env!("CARGO_PKG_VERSION"));
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1366.0, 820.0])
            .with_min_inner_size([1000.0, 680.0])
            .with_maximized(true)
            .with_title(&title)
            .with_icon(icon),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(&title, opts, Box::new(|cc| Ok(Box::new(app::Studio::new(cc)))))
}
