//! WASAPI audio: speaker (loopback) and microphone capture, device lists, and in-app playback output.
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;
use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

pub const RATE: u32 = 48000;
type BoxErr = Box<dyn std::error::Error + Send + Sync>;

fn init_com() {
    // Each audio thread gets its own COM apartment. "Already initialised" is fine.
    let _ = wasapi::initialize_mta();
}

#[derive(Clone, Debug)]
pub struct Device {
    pub name: String,
    pub id: String,
}

#[derive(Clone, Debug, Default)]
pub struct DeviceList {
    pub speakers: Vec<Device>,
    pub microphones: Vec<Device>,
    pub default_speaker: String,
    pub default_microphone: String,
}

/// Lists devices on a worker thread (the window thread uses a different COM mode,
/// which is why the old version always showed an empty device list).
pub fn list_devices() -> DeviceList {
    thread::spawn(|| {
        init_com();
        let mut out = DeviceList::default();
        if let Ok(en) = DeviceEnumerator::new() {
            for (mic, list, default) in [
                (false, &mut out.speakers, &mut out.default_speaker),
                (true, &mut out.microphones, &mut out.default_microphone),
            ] {
                let dir = if mic { Direction::Capture } else { Direction::Render };
                if let Ok(d) = en.get_default_device(&dir) {
                    *default = d.get_friendlyname().unwrap_or_default();
                }
                if let Ok(col) = en.get_device_collection(&dir) {
                    for d in col.into_iter().flatten() {
                        let name = d.get_friendlyname().unwrap_or_else(|_| "Audio device".into());
                        let id = d.get_id().unwrap_or_default();
                        list.push(Device { name, id });
                    }
                }
            }
        }
        out
    })
    .join()
    .unwrap_or_default()
}

pub struct CaptureResult {
    pub path: PathBuf,
    /// The moment the first audio sample was recorded (used to line audio up with video).
    pub started: Instant,
}

/// Records speakers (loopback) or a microphone to a 32-bit float stereo WAV until `stop` is set.
/// `device_id` None = Windows default device.
pub fn spawn_capture(mic: bool, device_id: Option<String>, output: PathBuf, stop: Arc<AtomicBool>) -> JoinHandle<Result<CaptureResult, String>> {
    thread::spawn(move || capture(mic, device_id, output, stop).map_err(|e| e.to_string()))
}

fn capture(mic: bool, device_id: Option<String>, output: PathBuf, stop: Arc<AtomicBool>) -> Result<CaptureResult, BoxErr> {
    init_com();
    let en = DeviceEnumerator::new()?;
    let device = match device_id.filter(|x| !x.trim().is_empty()) {
        Some(id) => en.get_device(&id)?,
        None => en.get_default_device(if mic { &Direction::Capture } else { &Direction::Render })?,
    };
    let mut client = device.get_iaudioclient()?;
    let format = WaveFormat::new(32, 32, &SampleType::Float, RATE as usize, 2, None);
    let (period, _) = client.get_device_period()?;
    // Capture direction on a speaker (render) device = loopback recording of what you hear.
    client.initialize_client(&format, &Direction::Capture, &StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: period.max(100_000) })?;
    let event = client.set_get_eventhandle()?;
    let reader = client.get_audiocaptureclient()?;
    let spec = hound::WavSpec { channels: 2, sample_rate: RATE, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
    let mut wav = hound::WavWriter::create(&output, spec)?;
    let mut buf = VecDeque::<u8>::new();
    client.start_stream()?;
    let started = Instant::now();
    let mut written: u64 = 0; // stereo frames written
    let write_silence = |wav: &mut hound::WavWriter<_>, frames: u64| -> Result<(), BoxErr> {
        for _ in 0..frames * 2 {
            wav.write_sample(0.0f32)?;
        }
        Ok(())
    };
    while !stop.load(Ordering::Relaxed) {
        if event.wait_for_event(40).is_ok() {
            let info = reader.read_from_device_to_deque(&mut buf)?;
            let frames = buf.len() / 8;
            if info.flags.silent {
                buf.drain(..frames * 8);
                write_silence(&mut wav, frames as u64)?;
            } else {
                for _ in 0..frames * 2 {
                    let b = [buf.pop_front().unwrap_or(0), buf.pop_front().unwrap_or(0), buf.pop_front().unwrap_or(0), buf.pop_front().unwrap_or(0)];
                    wav.write_sample(f32::from_le_bytes(b))?;
                }
            }
            written += frames as u64;
        }
        // Windows sends nothing at all from speakers while no sound is playing.
        // Fill those gaps with silence so the audio stays in sync with the video clock.
        let expected = (started.elapsed().as_secs_f64() * RATE as f64) as u64;
        if expected > written + RATE as u64 / 5 {
            let fill = expected - written - RATE as u64 / 20;
            write_silence(&mut wav, fill)?;
            written += fill;
        }
    }
    let expected = (started.elapsed().as_secs_f64() * RATE as f64) as u64;
    if expected > written {
        write_silence(&mut wav, expected - written)?;
    }
    let _ = client.stop_stream();
    wav.finalize()?; // writes the WAV header so the file plays everywhere
    Ok(CaptureResult { path: output, started })
}

/// Shared sample buffer between the decoder and the speaker output.
pub struct Ring {
    pub samples: Mutex<VecDeque<f32>>,
    pub decoder_done: AtomicBool,
}

/// Status of the playback output, read by the player for its clock.
pub struct OutputState {
    pub played_frames: AtomicU64,
    pub ok: AtomicBool,
    pub started: AtomicBool,
    pub finished: AtomicBool,
    pub volume: AtomicU32,
}

/// Plays 48 kHz stereo float samples from `ring` on the default speaker, inside this program.
pub fn spawn_output(ring: Arc<Ring>, state: Arc<OutputState>, stop: Arc<AtomicBool>) -> JoinHandle<()> {
    thread::spawn(move || {
        if let Err(_e) = output(&ring, &state, &stop) {
            state.ok.store(false, Ordering::Relaxed);
        }
        state.finished.store(true, Ordering::Relaxed);
    })
}

fn output(ring: &Ring, state: &OutputState, stop: &AtomicBool) -> Result<(), BoxErr> {
    init_com();
    let en = DeviceEnumerator::new()?;
    let device = en.get_default_device(&Direction::Render)?;
    let mut client = device.get_iaudioclient()?;
    let format = WaveFormat::new(32, 32, &SampleType::Float, RATE as usize, 2, None);
    client.initialize_client(&format, &Direction::Render, &StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: 400_000 })?;
    let event = client.set_get_eventhandle()?;
    let render = client.get_audiorenderclient()?;
    // Pre-buffer ~0.15 s (or until the file ends) so playback starts smoothly.
    loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let len = ring.samples.lock().map(|s| s.len()).unwrap_or(0);
        if len >= (RATE as usize * 2) * 15 / 100 || ring.decoder_done.load(Ordering::Relaxed) {
            break;
        }
        thread::sleep(std::time::Duration::from_millis(5));
    }
    client.start_stream()?;
    state.ok.store(true, Ordering::Relaxed);
    state.started.store(true, Ordering::Relaxed);
    let mut total: u64 = 0;
    let mut bytes: Vec<u8> = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        let _ = event.wait_for_event(100);
        let space = client.get_available_space_in_frames()? as usize;
        let vol = f32::from_bits(state.volume.load(Ordering::Relaxed));
        let mut frames = 0usize;
        bytes.clear();
        if let Ok(mut s) = ring.samples.lock() {
            frames = space.min(s.len() / 2);
            for _ in 0..frames * 2 {
                let v = s.pop_front().unwrap_or(0.0) * vol;
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        if frames > 0 {
            render.write_to_device(frames, &bytes, None)?;
            total += frames as u64;
        }
        let padding = client.get_current_padding().unwrap_or(0) as u64;
        state.played_frames.store(total.saturating_sub(padding), Ordering::Relaxed);
        let empty = ring.samples.lock().map(|s| s.is_empty()).unwrap_or(true);
        if empty && ring.decoder_done.load(Ordering::Relaxed) && padding == 0 {
            break; // everything has been heard
        }
    }
    let _ = client.stop_stream();
    Ok(())
}
