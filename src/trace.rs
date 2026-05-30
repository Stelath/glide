use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

use crate::benchmark::SpanRecord;

#[derive(Clone, Default)]
pub(crate) struct TraceSession {
    inner: Option<Arc<TraceState>>,
}

struct TraceState {
    run_id: String,
    started: Instant,
    file: Mutex<File>,
}

#[derive(Serialize)]
struct TraceEvent<'a> {
    schema_version: u8,
    run_id: &'a str,
    event: &'a str,
    name: &'a str,
    unix_ms: u128,
    at_ms: f64,
    duration_ms: Option<f64>,
    attrs: BTreeMap<String, String>,
}

impl TraceSession {
    pub(crate) fn from_env(label: &str) -> Self {
        let enabled = std::env::var_os("GLIDE_TRACE_PATH").is_some()
            || std::env::var("GLIDE_TRACE")
                .map(|value| value != "0" && !value.eq_ignore_ascii_case("false"))
                .unwrap_or(false);

        if !enabled {
            return Self::disabled();
        }

        let path = std::env::var_os("GLIDE_TRACE_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp/glide-trace.jsonl"));

        Self::for_path(label, path).unwrap_or_else(|error| {
            eprintln!("[glide] failed to initialize trace file: {error:#}");
            Self::disabled()
        })
    }

    pub(crate) fn disabled() -> Self {
        Self { inner: None }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    pub(crate) fn record(&self, name: &str, duration: Duration) {
        self.write_event(name, Some(duration), BTreeMap::new());
    }

    pub(crate) fn record_with_attrs(
        &self,
        name: &str,
        duration: Duration,
        attrs: BTreeMap<String, String>,
    ) {
        self.write_event(name, Some(duration), attrs);
    }

    pub(crate) fn record_since(&self, name: &str, started: Instant) {
        self.record(name, started.elapsed());
    }

    pub(crate) fn instant(&self, name: &str) {
        self.write_event(name, None, BTreeMap::new());
    }

    pub(crate) fn instant_with_attrs(&self, name: &str, attrs: BTreeMap<String, String>) {
        self.write_event(name, None, attrs);
    }

    pub(crate) fn measure<T>(&self, name: &str, f: impl FnOnce() -> T) -> T {
        let started = Instant::now();
        let result = f();
        self.record(name, started.elapsed());
        result
    }

    pub(crate) fn measure_result<T>(
        &self,
        name: &str,
        f: impl FnOnce() -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let started = Instant::now();
        let result = f();
        self.record(name, started.elapsed());
        result
    }

    pub(crate) fn record_profile_spans(&self, prefix: &str, spans: &[SpanRecord]) {
        if !self.is_enabled() {
            return;
        }

        for span in spans {
            self.record(
                &format!("{prefix}_{}", span.phase),
                Duration::from_secs_f64(span.duration_ms / 1000.0),
            );
        }
    }

    fn for_path(label: &str, path: PathBuf) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let session = Self {
            inner: Some(Arc::new(TraceState {
                run_id: format!("{label}-{}", unix_millis()),
                started: Instant::now(),
                file: Mutex::new(file),
            })),
        };
        session.instant("trace_start");
        Ok(session)
    }

    fn write_event(&self, name: &str, duration: Option<Duration>, attrs: BTreeMap<String, String>) {
        let Some(inner) = &self.inner else {
            return;
        };

        let event = TraceEvent {
            schema_version: 1,
            run_id: &inner.run_id,
            event: if duration.is_some() {
                "span"
            } else {
                "instant"
            },
            name,
            unix_ms: unix_millis(),
            at_ms: inner.started.elapsed().as_secs_f64() * 1000.0,
            duration_ms: duration.map(|d| d.as_secs_f64() * 1000.0),
            attrs,
        };

        let Ok(line) = serde_json::to_string(&event) else {
            return;
        };

        if let Ok(mut file) = inner.file.lock() {
            let _ = writeln!(file, "{line}");
        }
    }
}

pub(crate) fn attrs(
    pairs: impl IntoIterator<Item = (&'static str, String)>,
) -> BTreeMap<String, String> {
    pairs
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
