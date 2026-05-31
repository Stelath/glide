use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::{Context, Result};
use cpal::{
    Device, SampleFormat, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use crate::{
    app::state::LiveAudioData,
    app::trace::{TraceSession, attrs},
    config::AudioConfig,
};

const RING_BUFFER_SIZE: usize = 8192;
const DEFAULT_INPUT_DEVICE: &str = "default";

#[derive(Debug, Clone, Copy)]
pub enum AudioFormat {
    Wav,
}

#[derive(Debug, Clone)]
pub struct RecordedAudio {
    pub bytes: Vec<u8>,
    pub format: AudioFormat,
    pub sample_count: usize,
}

pub struct AudioRecorder {
    active: Option<ActiveRecording>,
}

struct ActiveRecording {
    stream: Stream,
    samples: Arc<Mutex<Vec<i16>>>,
    sample_rate: u32,
    live_audio: Arc<Mutex<LiveAudioData>>,
}

struct InputStreamConfig {
    stream: StreamConfig,
    sample_format: SampleFormat,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self { active: None }
    }

    pub fn start(&mut self, config: &AudioConfig) -> Result<Arc<Mutex<LiveAudioData>>> {
        anyhow::ensure!(self.active.is_none(), "recording already in progress");

        let device = resolve_input_device(config)?;
        let input_config = default_input_stream_config(&device)?;
        let sample_rate = input_config.stream.sample_rate.0;
        let samples = Arc::new(Mutex::new(Vec::new()));
        let live_audio = live_audio_buffer(sample_rate);
        let stream = build_input_stream(
            &device,
            &input_config.stream,
            input_config.sample_format,
            samples.clone(),
            live_audio.clone(),
        )?;
        stream.play().context("failed to start audio stream")?;

        self.active = Some(ActiveRecording {
            stream,
            samples,
            sample_rate,
            live_audio: live_audio.clone(),
        });

        Ok(live_audio)
    }

    pub fn stop(&mut self) -> Result<RecordedAudio> {
        self.stop_profiled(&TraceSession::disabled())
    }

    pub(crate) fn stop_profiled(&mut self, trace: &TraceSession) -> Result<RecordedAudio> {
        let total_started = Instant::now();
        let active = trace.measure_result("audio_stop_take_active", || {
            self.active.take().context("recording is not active")
        })?;

        let ActiveRecording {
            stream,
            samples,
            sample_rate,
            live_audio: _live_audio,
        } = active;

        trace.measure("audio_stop_drop_stream", || drop(stream));

        let samples = trace.measure("audio_stop_clone_samples", || {
            samples.lock().expect("samples poisoned").clone()
        });
        anyhow::ensure!(!samples.is_empty(), "no audio samples were captured");

        let bytes = trace.measure_result("audio_stop_wav_encode", || {
            encode_wav(&samples, sample_rate)
        })?;

        trace.record_with_attrs(
            "audio_stop_total",
            total_started.elapsed(),
            attrs([
                ("sample_count", samples.len().to_string()),
                ("byte_count", bytes.len().to_string()),
                ("sample_rate", sample_rate.to_string()),
            ]),
        );

        Ok(RecordedAudio {
            bytes,
            format: AudioFormat::Wav,
            sample_count: samples.len(),
        })
    }
}

pub fn list_input_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let mut devices = vec![DEFAULT_INPUT_DEVICE.to_string()];
    for device in host
        .input_devices()
        .context("failed to enumerate input devices")?
    {
        devices.push(
            device
                .name()
                .unwrap_or_else(|_| "unnamed input".to_string()),
        );
    }
    devices.sort();
    devices.dedup();
    Ok(devices)
}

fn resolve_input_device(config: &AudioConfig) -> Result<Device> {
    let host = cpal::default_host();

    if config.device == DEFAULT_INPUT_DEVICE {
        return host
            .default_input_device()
            .context("no default input device is available");
    }

    host.input_devices()
        .context("failed to enumerate input devices")?
        .find(|device| {
            device
                .name()
                .map(|name| name == config.device)
                .unwrap_or(false)
        })
        .with_context(|| format!("input device '{}' was not found", config.device))
}

fn default_input_stream_config(device: &Device) -> Result<InputStreamConfig> {
    // CPAL reports the OS/device preferred input format here. We preserve it for capture
    // and downmix to mono only in our recorded/live buffers.
    let supported = device
        .default_input_config()
        .context("failed to determine default input configuration")?;

    Ok(InputStreamConfig {
        stream: supported.config(),
        sample_format: supported.sample_format(),
    })
}

fn live_audio_buffer(sample_rate: u32) -> Arc<Mutex<LiveAudioData>> {
    Arc::new(Mutex::new(LiveAudioData {
        ring: vec![0.0; RING_BUFFER_SIZE],
        write_pos: 0,
        sample_rate,
    }))
}

fn build_input_stream(
    device: &Device,
    config: &StreamConfig,
    sample_format: SampleFormat,
    samples: Arc<Mutex<Vec<i16>>>,
    live_audio: Arc<Mutex<LiveAudioData>>,
) -> Result<Stream> {
    let channels = config.channels;

    Ok(match sample_format {
        SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _| {
                push_samples(data, channels, &samples, &live_audio, normalize_i16)
            },
            log_stream_error,
            None,
        )?,
        SampleFormat::U16 => device.build_input_stream(
            config,
            move |data: &[u16], _| {
                push_samples(data, channels, &samples, &live_audio, normalize_u16)
            },
            log_stream_error,
            None,
        )?,
        SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _| {
                push_samples(data, channels, &samples, &live_audio, normalize_f32)
            },
            log_stream_error,
            None,
        )?,
        other => anyhow::bail!("unsupported sample format: {other:?}"),
    })
}

fn log_stream_error(error: cpal::StreamError) {
    eprintln!("audio stream error: {error}");
}

fn encode_wav(samples: &[i16], sample_rate: u32) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::new(std::io::Cursor::new(&mut bytes), spec)
        .context("failed to create wav writer")?;
    for sample in samples {
        writer
            .write_sample(*sample)
            .context("failed to write wav sample")?;
    }
    writer.finalize().context("failed to finalize wav")?;

    Ok(bytes)
}

fn push_samples<S: Copy>(
    data: &[S],
    channels: u16,
    target: &Arc<Mutex<Vec<i16>>>,
    live: &Arc<Mutex<LiveAudioData>>,
    convert: impl Fn(S) -> f32,
) {
    let mut samples = target.lock().expect("samples poisoned");
    let mut live = live.lock().expect("live audio poisoned");
    push_frames(
        data.len(),
        usize::from(channels),
        |index| convert(data[index]),
        &mut samples,
        &mut live,
    );
}

fn normalize_i16(sample: i16) -> f32 {
    sample as f32 / i16::MAX as f32
}

fn normalize_u16(sample: u16) -> f32 {
    (sample as f32 / u16::MAX as f32) * 2.0 - 1.0
}

fn normalize_f32(sample: f32) -> f32 {
    sample
}

fn push_frames(
    len: usize,
    channels: usize,
    sample_at: impl Fn(usize) -> f32,
    target: &mut Vec<i16>,
    live: &mut LiveAudioData,
) {
    if channels == 0 {
        return;
    }

    let ring_len = live.ring.len();

    for frame_start in (0..len).step_by(channels) {
        let mut total = 0.0f32;
        let mut seen = 0usize;

        for channel in 0..channels {
            let index = frame_start + channel;
            if index >= len {
                break;
            }
            total += sample_at(index);
            seen += 1;
        }

        if seen == 0 {
            continue;
        }

        let averaged = (total / seen as f32).clamp(-1.0, 1.0);
        target.push((averaged * i16::MAX as f32) as i16);

        // Also write to the live ring buffer for the overlay EQ
        live.ring[live.write_pos % ring_len] = averaged;
        live.write_pos = live.write_pos.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};

    fn make_live() -> LiveAudioData {
        LiveAudioData {
            ring: vec![0.0; RING_BUFFER_SIZE],
            write_pos: 0,
            sample_rate: 16000,
        }
    }

    #[test]
    fn push_frames_mixes_clamps_and_handles_empty_channels() {
        let cases = [
            (
                vec![0.5, -0.5, 0.0],
                1,
                vec![
                    (0.5f32 * i16::MAX as f32) as i16,
                    (-(0.5f32 * i16::MAX as f32)) as i16,
                    0,
                ],
            ),
            (
                vec![0.4, 0.6, 1.0, -1.0],
                2,
                vec![(0.5f32 * i16::MAX as f32) as i16, 0],
            ),
            (
                vec![2.0, -2.0],
                1,
                vec![i16::MAX, (-(i16::MAX as f32)) as i16],
            ),
            (vec![0.0; 4], 0, Vec::new()),
        ];

        for (data, channels, expected) in cases {
            let mut target = Vec::new();
            let mut live = make_live();
            push_frames(data.len(), channels, |i| data[i], &mut target, &mut live);
            assert_eq!(target, expected);
            assert_eq!(live.write_pos, expected.len());
        }
    }

    #[test]
    fn sample_format_adapters_normalize_to_i16() {
        let target = Arc::new(Mutex::new(Vec::new()));
        let live = Arc::new(Mutex::new(make_live()));
        let data: Vec<i16> = vec![i16::MAX, i16::MIN];
        push_samples(&data, 1, &target, &live, |s| s as f32 / i16::MAX as f32);
        let samples = target.lock().unwrap();
        assert_eq!(samples.len(), 2);
        assert!(samples[0] > 0);
        assert!(samples[1] < 0);

        let target = Arc::new(Mutex::new(Vec::new()));
        let live = Arc::new(Mutex::new(make_live()));
        let data: Vec<u16> = vec![u16::MAX / 2];
        push_samples(&data, 1, &target, &live, |s| {
            (s as f32 / u16::MAX as f32) * 2.0 - 1.0
        });
        let samples = target.lock().unwrap();
        assert_eq!(samples.len(), 1);
        assert!(samples[0].abs() < 1000);

        let target = Arc::new(Mutex::new(Vec::new()));
        let live = Arc::new(Mutex::new(make_live()));
        let data: Vec<f32> = vec![0.0, 1.0, -1.0];
        push_samples(&data, 1, &target, &live, |s| s);
        let samples = target.lock().unwrap();
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[0], 0);
        assert_eq!(samples[1], i16::MAX);
        assert_eq!(samples[2], (-(i16::MAX as f32)) as i16);
    }

    #[test]
    fn encode_wav_round_trips_samples() {
        let expected = vec![0, i16::MAX, -1234];
        let bytes = encode_wav(&expected, 16_000).unwrap();
        let mut reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();

        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, 16_000);
        let actual = reader
            .samples::<i16>()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn ring_buffer_wraps() {
        let mut target = Vec::new();
        let mut live = LiveAudioData {
            ring: vec![0.0; 4],
            write_pos: 0,
            sample_rate: 16000,
        };

        push_frames(6, 1, |_| 0.5, &mut target, &mut live);

        assert_eq!(live.write_pos, 6);
        assert_eq!(target.len(), 6);
        assert!((live.ring[0] - 0.5).abs() < 0.001);
        assert!((live.ring[1] - 0.5).abs() < 0.001);
    }
}
