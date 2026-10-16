use std::collections::VecDeque;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use wasapi::{AudioClient, Direction, SampleType, StreamMode, WaveFormat};

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
const BITS_PER_SAMPLE: u16 = 32;
const BYTES_PER_FRAME: usize = CHANNELS as usize * (BITS_PER_SAMPLE as usize / 8);
pub const STREAM_SAMPLES_PER_CHANNEL: usize = SAMPLE_RATE as usize / 50;
pub const STREAM_SAMPLES_PER_FRAME: usize = STREAM_SAMPLES_PER_CHANNEL * CHANNELS as usize;
const STREAM_BYTES_PER_FRAME: usize = STREAM_SAMPLES_PER_FRAME * std::mem::size_of::<f32>();
const STREAM_QUEUE_CAPACITY: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureSummary {
    pub frames: usize,
    pub sample_rate: u32,
}

enum CaptureEvent {
    Frame(Vec<f32>),
    Error(String),
}

pub struct ApplicationCapture {
    events: Receiver<CaptureEvent>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl ApplicationCapture {
    pub fn try_frame(&self) -> Result<Option<Vec<f32>>, String> {
        match self.events.try_recv() {
            Ok(CaptureEvent::Frame(frame)) => Ok(Some(frame)),
            Ok(CaptureEvent::Error(error)) => Err(error),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err("audio capture stopped unexpectedly".to_string())
            }
        }
    }

    pub fn stop(mut self) {
        self.stop_and_join();
    }

    fn stop_and_join(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for ApplicationCapture {
    fn drop(&mut self) {
        self.stop_and_join();
    }
}

pub fn start_application_capture(process_id: u32) -> Result<ApplicationCapture, Box<dyn Error>> {
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let (events_tx, events_rx) = mpsc::sync_channel(STREAM_QUEUE_CAPACITY);
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), String>>(1);

    let worker = thread::Builder::new()
        .name("soundshare-audio-capture".to_string())
        .spawn(move || {
            if let Err(error) =
                capture_stream_worker(process_id, worker_stop, &events_tx, &ready_tx)
            {
                let message = error.to_string();
                let _ = ready_tx.try_send(Err(message.clone()));
                let _ = events_tx.try_send(CaptureEvent::Error(message));
            }
        })?;

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(ApplicationCapture {
            events: events_rx,
            stop,
            worker: Some(worker),
        }),
        Ok(Err(error)) => {
            let _ = worker.join();
            Err(error.into())
        }
        Err(_) => {
            let _ = worker.join();
            Err("audio capture failed before initialization completed".into())
        }
    }
}

fn capture_stream_worker(
    process_id: u32,
    stop: Arc<AtomicBool>,
    events: &SyncSender<CaptureEvent>,
    ready: &SyncSender<Result<(), String>>,
) -> Result<(), Box<dyn Error>> {
    wasapi::initialize_mta().ok()?;

    let desired_format = WaveFormat::new(
        BITS_PER_SAMPLE as usize,
        BITS_PER_SAMPLE as usize,
        &SampleType::Float,
        SAMPLE_RATE as usize,
        CHANNELS as usize,
        None,
    );
    let mut audio_client = AudioClient::new_application_loopback_client(process_id, true)?;
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: 0,
    };
    audio_client.initialize_client(&desired_format, &Direction::Capture, &mode)?;

    let event = audio_client.set_get_eventhandle()?;
    let capture_client = audio_client.get_audiocaptureclient()?;
    let mut captured_bytes = VecDeque::<u8>::new();

    audio_client.start_stream()?;
    ready
        .send(Ok(()))
        .map_err(|_| "capture startup receiver closed")?;

    while !stop.load(Ordering::Acquire) {
        let _ = event.wait_for_event(100);

        loop {
            let available_frames = capture_client.get_next_packet_size()?.unwrap_or(0);
            if available_frames == 0 {
                break;
            }
            capture_client.read_from_device_to_deque(&mut captured_bytes)?;
        }

        while captured_bytes.len() >= STREAM_BYTES_PER_FRAME {
            let mut bytes = vec![0_u8; STREAM_BYTES_PER_FRAME];
            for byte in &mut bytes {
                *byte = captured_bytes
                    .pop_front()
                    .expect("frame length was checked");
            }
            let frame = float_samples_from_bytes(&bytes);
            match events.try_send(CaptureEvent::Frame(frame)) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => {
                    stop.store(true, Ordering::Release);
                    break;
                }
            }
        }
    }

    audio_client.stop_stream()?;
    Ok(())
}

fn float_samples_from_bytes(bytes: &[u8]) -> Vec<f32> {
    let (samples, remainder) = bytes.as_chunks::<4>();
    debug_assert!(remainder.is_empty());
    samples
        .iter()
        .map(|sample| f32::from_le_bytes(*sample))
        .collect()
}

pub fn capture_application_to_wav(
    process_id: u32,
    duration: Duration,
    output_path: &Path,
) -> Result<CaptureSummary, Box<dyn Error>> {
    wasapi::initialize_mta().ok()?;

    let desired_format = WaveFormat::new(
        BITS_PER_SAMPLE as usize,
        BITS_PER_SAMPLE as usize,
        &SampleType::Float,
        SAMPLE_RATE as usize,
        CHANNELS as usize,
        None,
    );
    let mut audio_client = AudioClient::new_application_loopback_client(process_id, true)?;
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: 0,
    };
    audio_client.initialize_client(&desired_format, &Direction::Capture, &mode)?;

    let event = audio_client.set_get_eventhandle()?;
    let capture_client = audio_client.get_audiocaptureclient()?;
    let mut captured_bytes = VecDeque::<u8>::new();

    audio_client.start_stream()?;
    let started_at = Instant::now();
    while started_at.elapsed() < duration {
        let _ = event.wait_for_event(100);

        loop {
            let available_frames = capture_client.get_next_packet_size()?.unwrap_or(0);
            if available_frames == 0 {
                break;
            }

            capture_client.read_from_device_to_deque(&mut captured_bytes)?;
        }
    }
    audio_client.stop_stream()?;

    let audio_data: Vec<u8> = captured_bytes.into_iter().collect();
    write_float_wav(output_path, &audio_data)?;

    Ok(CaptureSummary {
        frames: audio_data.len() / BYTES_PER_FRAME,
        sample_rate: SAMPLE_RATE,
    })
}

fn write_float_wav(path: &Path, audio_data: &[u8]) -> Result<(), Box<dyn Error>> {
    let data_size: u32 = audio_data
        .len()
        .try_into()
        .map_err(|_| "captured audio is too large for a WAV file")?;
    let riff_size = 36_u32
        .checked_add(data_size)
        .ok_or("captured audio is too large for a WAV file")?;
    let block_align = CHANNELS * (BITS_PER_SAMPLE / 8);
    let byte_rate = SAMPLE_RATE * u32::from(block_align);

    let mut writer = BufWriter::new(File::create(path)?);
    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_size.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16_u32.to_le_bytes())?;
    writer.write_all(&3_u16.to_le_bytes())?; // WAVE_FORMAT_IEEE_FLOAT
    writer.write_all(&CHANNELS.to_le_bytes())?;
    writer.write_all(&SAMPLE_RATE.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&BITS_PER_SAMPLE.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_size.to_le_bytes())?;
    writer.write_all(audio_data)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn wav_writer_creates_a_valid_float_header() {
        let unique_suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "soundshare-wav-test-{}-{unique_suffix}.wav",
            std::process::id(),
        ));
        let samples = [0_u8; BYTES_PER_FRAME * 4];

        write_float_wav(&path, &samples).expect("test WAV should be written");
        let bytes = fs::read(&path).expect("test WAV should be readable");
        let _ = fs::remove_file(&path);

        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), 3);
        assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), CHANNELS);
        assert_eq!(
            u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
            SAMPLE_RATE
        );
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(bytes.len(), 44 + samples.len());
    }

    #[test]
    fn capture_bytes_are_converted_to_float_samples() {
        let values = [0.0_f32, 0.5, -0.25, 1.0];
        let bytes: Vec<u8> = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();

        assert_eq!(float_samples_from_bytes(&bytes), values);
    }
}
