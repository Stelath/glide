use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Result;

use super::types::SpanRecord;

#[derive(Clone, Default)]
pub(crate) struct ProfileCollector {
    inner: Option<Arc<ProfileState>>,
}

#[derive(Default)]
struct ProfileState {
    spans: Mutex<Vec<SpanRecord>>,
    markers: Mutex<BTreeMap<String, Instant>>,
}

impl ProfileCollector {
    pub(crate) fn enabled() -> Self {
        Self {
            inner: Some(Arc::new(ProfileState::default())),
        }
    }

    pub(crate) fn disabled() -> Self {
        Self { inner: None }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    pub(crate) fn record(&self, phase: impl Into<String>, duration: Duration) {
        if let Some(inner) = &self.inner
            && let Ok(mut spans) = inner.spans.lock()
        {
            spans.push(SpanRecord {
                phase: phase.into(),
                duration_ms: duration.as_secs_f64() * 1000.0,
            });
        }
    }

    pub(crate) fn measure<T>(&self, phase: &str, f: impl FnOnce() -> T) -> T {
        let started = Instant::now();
        let result = f();
        self.record(phase, started.elapsed());
        result
    }

    pub(crate) fn measure_result<T>(
        &self,
        phase: &str,
        f: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let started = Instant::now();
        let result = f();
        self.record(phase, started.elapsed());
        result
    }

    pub(crate) fn mark(&self, marker: impl Into<String>) {
        if let Some(inner) = &self.inner
            && let Ok(mut markers) = inner.markers.lock()
        {
            markers.insert(marker.into(), Instant::now());
        }
    }

    pub(crate) fn record_since_marker(&self, marker: &str, phase: impl Into<String>) {
        let Some(started) = self.inner.as_ref().and_then(|inner| {
            inner
                .markers
                .lock()
                .ok()
                .and_then(|markers| markers.get(marker).copied())
        }) else {
            return;
        };
        self.record(phase, started.elapsed());
    }

    pub(crate) fn spans(&self) -> Vec<SpanRecord> {
        self.inner
            .as_ref()
            .and_then(|inner| inner.spans.lock().ok().map(|spans| spans.clone()))
            .unwrap_or_default()
    }
}
