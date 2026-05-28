use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig, Wave};

use crate::{audio::AudioFormat, benchmark::ProfileCollector, local_models};

type SharedRecognizer = Arc<Mutex<OfflineRecognizer>>;

static RECOGNIZERS: OnceLock<Mutex<HashMap<String, SharedRecognizer>>> = OnceLock::new();

pub struct ParakeetSttProvider {
    model_id: String,
    recognizer: SharedRecognizer,
    profile: ProfileCollector,
}

impl ParakeetSttProvider {
    pub fn new(model_id: &str, profile: ProfileCollector) -> Result<Self> {
        let recognizer = recognizer_for_model(model_id, &profile)?;
        Ok(Self {
            model_id: model_id.to_string(),
            recognizer,
            profile,
        })
    }
}

pub(crate) fn prewarm_model(model_id: &str) -> Result<()> {
    recognizer_for_model(model_id, &ProfileCollector::disabled()).map(|_| ())
}

#[async_trait::async_trait]
impl super::SttProvider for ParakeetSttProvider {
    async fn transcribe(&self, audio: &[u8], format: AudioFormat) -> Result<String> {
        match format {
            AudioFormat::Wav => {}
        }

        let audio = audio.to_vec();
        let model_id = self.model_id.clone();
        let recognizer = self.recognizer.clone();
        let profile = self.profile.clone();
        tokio::task::spawn_blocking(move || {
            transcribe_wav_bytes(&model_id, recognizer, &audio, &profile)
        })
        .await?
    }

    fn name(&self) -> &'static str {
        "Parakeet"
    }
}

fn recognizer_for_model(model_id: &str, profile: &ProfileCollector) -> Result<SharedRecognizer> {
    let cache = RECOGNIZERS.get_or_init(|| Mutex::new(HashMap::new()));
    let lock_started = std::time::Instant::now();
    let mut locked = cache.lock().expect("recognizer cache poisoned");
    profile.record("parakeet_recognizer_cache_lock", lock_started.elapsed());
    if let Some(recognizer) = locked.get(model_id) {
        profile.record("parakeet_recognizer_cache_hit", std::time::Duration::ZERO);
        return Ok(recognizer.clone());
    }

    let recognizer = Arc::new(Mutex::new(
        profile.measure_result("parakeet_recognizer_build", || create_recognizer(model_id))?,
    ));
    locked.insert(model_id.to_string(), recognizer.clone());
    Ok(recognizer)
}

fn create_recognizer(model_id: &str) -> Result<OfflineRecognizer> {
    let model_dir = local_models::parakeet_model_dir(model_id)?;
    local_models::validate_parakeet_model_dir(&model_dir)?;

    let mut config = OfflineRecognizerConfig::default();
    config.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(
            model_dir
                .join("encoder.int8.onnx")
                .to_string_lossy()
                .to_string(),
        ),
        decoder: Some(
            model_dir
                .join("decoder.int8.onnx")
                .to_string_lossy()
                .to_string(),
        ),
        joiner: Some(
            model_dir
                .join("joiner.int8.onnx")
                .to_string_lossy()
                .to_string(),
        ),
    };
    config.model_config.tokens = Some(model_dir.join("tokens.txt").to_string_lossy().to_string());
    config.model_config.provider = Some("cpu".to_string());
    config.model_config.num_threads = 2;
    config.model_config.debug = false;

    OfflineRecognizer::create(&config).context("failed to create Parakeet recognizer")
}

fn transcribe_wav_bytes(
    model_id: &str,
    recognizer: SharedRecognizer,
    audio: &[u8],
    profile: &ProfileCollector,
) -> Result<String> {
    let path = profile.measure_result("parakeet_temp_wav_write", || {
        write_temp_audio(model_id, audio)
    })?;
    let result = (|| -> Result<String> {
        let wave = profile.measure_result("parakeet_temp_wav_read", || {
            Wave::read(path.to_string_lossy().as_ref())
                .context("failed to read recorded WAV for Parakeet")
        })?;
        let lock_started = std::time::Instant::now();
        let recognizer = recognizer.lock().expect("Parakeet recognizer poisoned");
        profile.record("parakeet_recognizer_lock_wait", lock_started.elapsed());
        let stream = profile.measure("parakeet_stream_create", || recognizer.create_stream());
        stream.accept_waveform(wave.sample_rate(), wave.samples());
        profile.record_since_marker("flow_release", "flow_release_to_stt_decode_start");
        profile.measure("parakeet_decode", || recognizer.decode(&stream));
        let result = profile.measure_result("parakeet_result_extract", || {
            stream
                .get_result()
                .context("Parakeet recognizer did not return a result")
        })?;
        Ok(result.text.trim().to_string())
    })();
    std::fs::remove_file(path).ok();
    result
}

fn write_temp_audio(model_id: &str, audio: &[u8]) -> Result<PathBuf> {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("glide-{model_id}-{suffix}.wav"));
    std::fs::write(&path, audio)
        .with_context(|| format!("failed to write temporary audio to {}", path.display()))?;
    Ok(path)
}
