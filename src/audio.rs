use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, SampleFormat, Stream, StreamConfig,
};

use crate::config::AudioConfig;

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
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self { active: None }
    }

    pub fn start(&mut self, config: &AudioConfig) -> Result<()> {
        anyhow::ensure!(self.active.is_none(), "recording already in progress");

        let device = resolve_input_device(config)?;
        let supported = device
            .default_input_config()
            .context("failed to determine default input configuration")?;

        let sample_rate = supported.sample_rate().0;

        let stream_config = StreamConfig {
            channels: supported.channels(),
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let samples = Arc::new(Mutex::new(Vec::new()));
        let capture_target = samples.clone();
        let channels = stream_config.channels;

        let error_callback = |error| {
            eprintln!("audio stream error: {error}");
        };

        let stream = match supported.sample_format() {
            SampleFormat::I16 => device.build_input_stream(
                &stream_config,
                move |data: &[i16], _| push_i16_frames(data, channels, &capture_target),
                error_callback,
                None,
            )?,
            SampleFormat::U16 => device.build_input_stream(
                &stream_config,
                move |data: &[u16], _| push_u16_frames(data, channels, &capture_target),
                error_callback,
                None,
            )?,
            SampleFormat::F32 => device.build_input_stream(
                &stream_config,
                move |data: &[f32], _| push_f32_frames(data, channels, &capture_target),
                error_callback,
                None,
            )?,
            other => anyhow::bail!("unsupported sample format: {other:?}"),
        };

        stream.play().context("failed to start audio stream")?;

        self.active = Some(ActiveRecording {
            stream,
            samples,
            sample_rate,
        });

        Ok(())
    }

    pub fn stop(&mut self) -> Result<RecordedAudio> {
        let active = self.active.take().context("recording is not active")?;
        drop(active.stream);

        let samples = active.samples.lock().expect("samples poisoned").clone();
        anyhow::ensure!(!samples.is_empty(), "no audio samples were captured");

        let mut bytes = Vec::new();
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: active.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = hound::WavWriter::new(std::io::Cursor::new(&mut bytes), spec)
            .context("failed to create wav writer")?;
        for sample in &samples {
            writer
                .write_sample(*sample)
                .context("failed to write wav sample")?;
        }
        writer.finalize().context("failed to finalize wav")?;

        Ok(RecordedAudio {
            bytes,
            format: AudioFormat::Wav,
            sample_count: samples.len(),
        })
    }
}

pub fn list_input_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let mut devices = vec!["default".to_string()];
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

    if config.device == "default" {
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

fn push_i16_frames(data: &[i16], channels: u16, target: &Arc<Mutex<Vec<i16>>>) {
    let mut samples = target.lock().expect("samples poisoned");
    push_frames(
        data.len(),
        usize::from(channels),
        |index| data[index] as f32 / i16::MAX as f32,
        &mut samples,
    );
}

fn push_u16_frames(data: &[u16], channels: u16, target: &Arc<Mutex<Vec<i16>>>) {
    let mut samples = target.lock().expect("samples poisoned");
    push_frames(
        data.len(),
        usize::from(channels),
        |index| (data[index] as f32 / u16::MAX as f32) * 2.0 - 1.0,
        &mut samples,
    );
}

fn push_f32_frames(data: &[f32], channels: u16, target: &Arc<Mutex<Vec<i16>>>) {
    let mut samples = target.lock().expect("samples poisoned");
    push_frames(
        data.len(),
        usize::from(channels),
        |index| data[index],
        &mut samples,
    );
}

fn push_frames(
    len: usize,
    channels: usize,
    sample_at: impl Fn(usize) -> f32,
    target: &mut Vec<i16>,
) {
    if channels == 0 {
        return;
    }

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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_push_frames_single_channel() {
        let mut target = Vec::new();
        push_frames(3, 1, |i| [0.5, -0.5, 0.0][i], &mut target);
        assert_eq!(target.len(), 3);
        // 0.5 * 32767 ≈ 16383
        assert_eq!(target[0], (0.5f32 * i16::MAX as f32) as i16);
        assert_eq!(target[1], (-0.5f32 * i16::MAX as f32) as i16);
        assert_eq!(target[2], 0);
    }

    #[test]
    fn test_push_frames_stereo_averages() {
        let mut target = Vec::new();
        // Two frames: [0.4, 0.6] and [1.0, -1.0]
        let data = [0.4f32, 0.6, 1.0, -1.0];
        push_frames(4, 2, |i| data[i], &mut target);
        assert_eq!(target.len(), 2);
        // First frame average: (0.4+0.6)/2 = 0.5
        assert_eq!(target[0], (0.5f32 * i16::MAX as f32) as i16);
        // Second frame average: (1.0+(-1.0))/2 = 0.0
        assert_eq!(target[1], 0);
    }

    #[test]
    fn test_push_frames_zero_channels() {
        let mut target = Vec::new();
        push_frames(4, 0, |_| 0.0, &mut target);
        assert!(target.is_empty());
    }

    #[test]
    fn test_push_i16_frames() {
        let target = Arc::new(Mutex::new(Vec::new()));
        let data: Vec<i16> = vec![i16::MAX, i16::MIN];
        push_i16_frames(&data, 1, &target);
        let samples = target.lock().unwrap();
        assert_eq!(samples.len(), 2);
        // i16::MAX normalized to 1.0 then back
        assert!(samples[0] > 0);
        assert!(samples[1] < 0);
    }

    #[test]
    fn test_push_u16_frames() {
        let target = Arc::new(Mutex::new(Vec::new()));
        // u16 midpoint (32768) should map to ~0.0
        let data: Vec<u16> = vec![u16::MAX / 2];
        push_u16_frames(&data, 1, &target);
        let samples = target.lock().unwrap();
        assert_eq!(samples.len(), 1);
        // Midpoint maps to near zero
        assert!(samples[0].abs() < 1000);
    }

    #[test]
    fn test_push_f32_frames() {
        let target = Arc::new(Mutex::new(Vec::new()));
        let data: Vec<f32> = vec![0.0, 1.0, -1.0];
        push_f32_frames(&data, 1, &target);
        let samples = target.lock().unwrap();
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[0], 0);
        assert_eq!(samples[1], i16::MAX);
        assert_eq!(samples[2], (-1.0f32 * i16::MAX as f32) as i16);
    }

    #[test]
    fn test_push_frames_clamps() {
        let mut target = Vec::new();
        // Values beyond [-1, 1] should be clamped
        push_frames(2, 1, |i| [2.0f32, -2.0][i], &mut target);
        assert_eq!(target.len(), 2);
        assert_eq!(target[0], i16::MAX);
        assert_eq!(target[1], (-1.0f32 * i16::MAX as f32) as i16);
    }
}
