//! Background jobs (exports, renders, saving recordings) so the window never freezes.
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

pub struct JobCtl {
    progress: Mutex<f32>,
    detail: Mutex<String>,
    cancel: AtomicBool,
}
impl JobCtl {
    pub fn set_progress(&self, p: f32) {
        if let Ok(mut v) = self.progress.lock() {
            *v = p.clamp(0.0, 1.0);
        }
    }
    pub fn set_detail(&self, d: impl Into<String>) {
        if let Ok(mut v) = self.detail.lock() {
            *v = d.into();
        }
    }
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// What a finished job reports back.
pub struct Done {
    pub message: String,
    /// File to select in the library afterwards.
    pub select: Option<PathBuf>,
    /// Command log for the Export page.
    pub log: String,
}
impl Done {
    #[allow(dead_code)]
    pub fn msg(m: impl Into<String>) -> Self {
        Self { message: m.into(), select: None, log: String::new() }
    }
}

pub struct Job {
    pub label: String,
    pub cancellable: bool,
    ctl: Arc<JobCtl>,
    rx: Receiver<Result<Done, String>>,
}

impl Job {
    pub fn spawn<F>(label: impl Into<String>, cancellable: bool, ctx: &eframe::egui::Context, f: F) -> Job
    where
        F: FnOnce(&JobCtl) -> Result<Done, String> + Send + 'static,
    {
        let ctl = Arc::new(JobCtl { progress: Mutex::new(0.0), detail: Mutex::new(String::new()), cancel: AtomicBool::new(false) });
        let (tx, rx) = channel();
        let c = ctl.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&c)))
                .unwrap_or_else(|_| Err("Internal error while processing (the job crashed).".into()));
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        Job { label: label.into(), cancellable, ctl, rx }
    }
    pub fn progress(&self) -> f32 {
        self.ctl.progress.lock().map(|v| *v).unwrap_or(0.0)
    }
    pub fn detail(&self) -> String {
        self.ctl.detail.lock().map(|v| v.clone()).unwrap_or_default()
    }
    pub fn cancel(&self) {
        self.ctl.cancel.store(true, Ordering::Relaxed);
    }
    /// Returns the result once the job has finished.
    pub fn poll(&self) -> Option<Result<Done, String>> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("Job ended unexpectedly".into())),
        }
    }
}
