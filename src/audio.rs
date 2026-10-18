use std::{collections::VecDeque, sync::{Arc,atomic::{AtomicBool,Ordering}}, thread, time::Duration, path::PathBuf};
use wasapi::{DeviceEnumerator, Direction, WaveFormat, SampleType, StreamMode, initialize_mta};

pub const RATE: u32 = 48000;
pub fn spawn_capture(mic: bool, device_id: Option<String>, output: PathBuf, stop: Arc<AtomicBool>) -> thread::JoinHandle<Result<(), String>> {
    thread::spawn(move || {
        capture_inner(mic, device_id, output, stop).map_err(|e| e.to_string())
    })
}
fn capture_inner(mic: bool, device_id: Option<String>, output: PathBuf, stop: Arc<AtomicBool>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    initialize_mta().ok().map_err(|e| std::io::Error::other(e.to_string()))?;
    let enumerator = DeviceEnumerator::new()?;
    let device = if let Some(id) = device_id.filter(|x| !x.trim().is_empty()) {
        enumerator.get_device(&id)?
    } else {
        enumerator.get_default_device(if mic {&Direction::Capture} else {&Direction::Render})?
    };
    let mut client = device.get_iaudioclient()?;
    let format = WaveFormat::new(32, 32, &SampleType::Float, RATE as usize, 2, None);
    let (period, _) = client.get_device_period()?;
    client.initialize_client(&format, &Direction::Capture, &StreamMode::EventsShared { autoconvert:true, buffer_duration_hns:period })?;
    let event = client.set_get_eventhandle()?;
    let reader = client.get_audiocaptureclient()?;
    let spec = hound::WavSpec{ channels:2, sample_rate: RATE, bits_per_sample:32, sample_format:hound::SampleFormat::Float };
    let mut wav = hound::WavWriter::create(output, spec)?;
    let mut buf = VecDeque::<u8>::new();
    client.start_stream()?;
    while !stop.load(Ordering::Relaxed) {
        if event.wait_for_event(150).is_err() { continue; }
        reader.read_from_device_to_deque(&mut buf)?;
        while buf.len() >= 4 {
            let bytes = [buf.pop_front().unwrap(),buf.pop_front().unwrap(),buf.pop_front().unwrap(),buf.pop_front().unwrap()];
            wav.write_sample(f32::from_le_bytes(bytes))?;
        }
    }
    client.stop_stream()?;
    // WAV writer finalizes its header to keep recordings playable.
    wav.finalize()?;
    Ok(())
}

pub fn list_devices(mic: bool) -> Result<Vec<(String,String)>, String> {
    initialize_mta().ok().map_err(|e| e.to_string())?;
    let en = DeviceEnumerator::new().map_err(|e| e.to_string())?;
    let col = en.get_device_collection(if mic { &Direction::Capture } else { &Direction::Render }).map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for device in col.into_iter() {
        let name = device.get_friendlyname().unwrap_or_else(|_| "Audio device".into());
        let id = device.get_id().unwrap_or_default();
        result.push((name,id));
    }
    Ok(result)
}
