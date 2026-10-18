//! Non-Windows stand-in for audio.rs (design preview build only): no sound devices.
#![allow(dead_code)]
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;
pub const RATE: u32 = 48000;
#[derive(Clone, Debug)]
pub struct Device { pub name: String, pub id: String }
#[derive(Clone, Debug, Default)]
pub struct DeviceList { pub speakers: Vec<Device>, pub microphones: Vec<Device>, pub default_speaker: String, pub default_microphone: String }
pub fn list_devices() -> DeviceList {
    DeviceList { default_speaker: "Realtek(R) Audio".into(), default_microphone: "USB Audio Device".into(), ..Default::default() }
}
pub struct CaptureResult { pub path: PathBuf, pub started: Instant }
pub fn spawn_capture(_mic: bool, _id: Option<String>, _out: PathBuf, _stop: Arc<AtomicBool>) -> JoinHandle<Result<CaptureResult, String>> {
    std::thread::spawn(|| Err("No audio on this platform".into()))
}
pub struct Ring { pub samples: Mutex<VecDeque<f32>>, pub decoder_done: AtomicBool }
pub struct OutputState { pub played_frames: AtomicU64, pub ok: AtomicBool, pub started: AtomicBool, pub finished: AtomicBool, pub volume: AtomicU32 }
pub fn spawn_output(_r: Arc<Ring>, state: Arc<OutputState>, _s: Arc<AtomicBool>) -> JoinHandle<()> {
    std::thread::spawn(move || state.finished.store(true, Ordering::Relaxed))
}
