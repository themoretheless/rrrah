//! Low-overhead, process-local telemetry for RAW benchmark runs.
//!
//! The recorder writes one JSON object per line and mirrors span events to the
//! Chrome trace event model.  A bounded broadcast channel is deliberately not
//! used here: benchmark correctness must not depend on a dashboard consumer.
//! Slow consumers simply stop receiving live events while the durable JSONL
//! and trace files continue to be written.
#![allow(
    clippy::cast_precision_loss,
    clippy::collapsible_if,
    clippy::match_same_arms,
    clippy::missing_errors_doc
)]

use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::Serialize;

const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct TelemetryEvent {
    pub schema: u32,
    pub run_id: String,
    pub sequence: u64,
    /// Monotonic nanoseconds since recorder creation.
    pub ts_ns: u128,
    pub kind: EventKind,
    pub name: String,
    pub stage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ns: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    SpanBegin,
    SpanEnd,
    Counter,
    Gauge,
}

#[derive(Debug)]
struct State {
    started: Instant,
    run_id: String,
    sequence: u64,
    jsonl: Option<BufWriter<File>>,
    trace: Option<BufWriter<File>>,
    // Bounded subscribers are deliberately best effort. A dashboard that is
    // slower than the producer must lose live samples, never back-pressure a
    // decoder or renderer.
    listeners: Vec<mpsc::SyncSender<String>>,
}

/// Thread-safe recorder. `emit` performs one JSON serialization and one
/// buffered append; listeners are best-effort and are never allowed to block
/// the benchmark pipeline. Files are flushed when the final recorder drops.
#[derive(Debug, Clone)]
pub struct Telemetry {
    state: Arc<Mutex<State>>,
}

impl Telemetry {
    pub fn new(run_id: impl Into<String>, jsonl: Option<&Path>, trace: Option<&Path>) -> Result<Self> {
        let jsonl = jsonl
            .map(open_append)
            .transpose()
            .context("open telemetry JSONL")?
            .map(BufWriter::new);
        let trace = trace
            .map(open_append)
            .transpose()
            .context("open Chrome trace")?
            .map(BufWriter::new);
        let recorder = Self {
            state: Arc::new(Mutex::new(State {
                started: Instant::now(),
                run_id: run_id.into(),
                sequence: 0,
                jsonl,
                trace,
                listeners: Vec::new(),
            })),
        };
        if let Ok(mut state) = recorder.state.lock() {
            if let Some(trace) = state.trace.as_mut() {
                trace.write_all(b"[\n").context("write trace header")?;
            }
        }
        Ok(recorder)
    }

    /// Subscribe to best-effort live JSONL lines. A full or disconnected
    /// receiver is removed on the next event.
    pub fn subscribe(&self) -> mpsc::Receiver<String> {
        let (sender, receiver) = mpsc::sync_channel(256);
        if let Ok(mut state) = self.state.lock() {
            state.listeners.push(sender);
        }
        receiver
    }

    pub fn span(&self, name: impl Into<String>, stage: impl Into<String>) -> SpanGuard {
        let name = name.into();
        let stage = stage.into();
        let started = Instant::now();
        self.emit(TelemetryEvent {
            schema: SCHEMA,
            run_id: String::new(),
            sequence: 0,
            ts_ns: 0,
            kind: EventKind::SpanBegin,
            name: name.clone(),
            stage: stage.clone(),
            duration_ns: None,
            value: None,
            unit: None,
            args: BTreeMap::new(),
        });
        SpanGuard {
            telemetry: self.clone(),
            name,
            stage,
            started,
            closed: false,
        }
    }

    pub fn counter(
        &self,
        name: impl Into<String>,
        stage: impl Into<String>,
        value: f64,
        unit: impl Into<String>,
    ) {
        self.emit(TelemetryEvent {
            schema: SCHEMA,
            run_id: String::new(),
            sequence: 0,
            ts_ns: 0,
            kind: EventKind::Counter,
            name: name.into(),
            stage: stage.into(),
            duration_ns: None,
            value: Some(value),
            unit: Some(unit.into()),
            args: BTreeMap::new(),
        });
    }

    pub fn gauge(
        &self,
        name: impl Into<String>,
        stage: impl Into<String>,
        value: f64,
        unit: impl Into<String>,
    ) {
        self.emit(TelemetryEvent {
            schema: SCHEMA,
            run_id: String::new(),
            sequence: 0,
            ts_ns: 0,
            kind: EventKind::Gauge,
            name: name.into(),
            stage: stage.into(),
            duration_ns: None,
            value: Some(value),
            unit: Some(unit.into()),
            args: BTreeMap::new(),
        });
    }

    fn emit(&self, mut event: TelemetryEvent) {
        let Ok(mut state) = self.state.lock() else { return };
        state.sequence = state.sequence.saturating_add(1);
        event.run_id.clone_from(&state.run_id);
        event.sequence = state.sequence;
        event.ts_ns = state.started.elapsed().as_nanos();
        let sequence = state.sequence;
        let Ok(line) = serde_json::to_string(&event) else {
            return;
        };
        if let Some(jsonl) = state.jsonl.as_mut() {
            let _ = writeln!(jsonl, "{line}");
        }
        if let Some(trace) = state.trace.as_mut() {
            let trace_event = chrome_event(&event);
            if sequence > 1 {
                let _ = trace.write_all(b",\n");
            }
            if let Ok(serialized) = serde_json::to_string(&trace_event) {
                let _ = trace.write_all(serialized.as_bytes());
            }
        }
        state
            .listeners
            .retain(|sender| sender.try_send(line.clone()).is_ok());
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        if Arc::strong_count(&self.state) != 1 {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            if let Some(trace) = state.trace.as_mut() {
                let _ = trace.write_all(b"\n]\n");
                let _ = trace.flush();
            }
            if let Some(jsonl) = state.jsonl.as_mut() {
                let _ = jsonl.flush();
            }
        }
    }
}

#[derive(Debug)]
pub struct SpanGuard {
    telemetry: Telemetry,
    name: String,
    stage: String,
    started: Instant,
    closed: bool,
}

impl SpanGuard {
    pub fn finish(mut self) {
        self.close();
    }
    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.telemetry.emit(TelemetryEvent {
            schema: SCHEMA,
            run_id: String::new(),
            sequence: 0,
            ts_ns: 0,
            kind: EventKind::SpanEnd,
            name: self.name.clone(),
            stage: self.stage.clone(),
            duration_ns: Some(self.started.elapsed().as_nanos()),
            value: None,
            unit: Some("ns".into()),
            args: BTreeMap::new(),
        });
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        self.close();
    }
}

#[derive(Debug, Serialize)]
struct ChromeEvent {
    name: String,
    cat: String,
    ph: String,
    ts: f64,
    pid: u32,
    tid: u64,
    args: BTreeMap<String, String>,
}

fn chrome_event(event: &TelemetryEvent) -> ChromeEvent {
    let phase = match event.kind {
        // SpanBegin/SpanEnd are paired events; using B/E makes Chrome trace
        // calculate duration correctly instead of emitting two unrelated
        // instant events. SpanEnd's duration_ns remains useful in JSONL.
        EventKind::SpanBegin => "B",
        EventKind::SpanEnd => "E",
        EventKind::Counter | EventKind::Gauge => "C",
    };
    let mut args = event.args.clone();
    if let Some(value) = event.value {
        args.insert("value".into(), value.to_string());
    }
    ChromeEvent {
        name: event.name.clone(),
        cat: event.stage.clone(),
        ph: phase.into(),
        ts: event.ts_ns as f64 / 1_000.0,
        pid: std::process::id(),
        // Stable numeric thread IDs are not portable on stable Rust; Chrome
        // trace still remains useful with one process-wide lane.
        tid: 0,
        args,
    }
}

fn open_append(path: &Path) -> Result<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create telemetry directory")?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))
}

pub fn default_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis();
    format!("run-{millis}-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emits_jsonl_and_closes_trace() {
        let dir = tempfile::tempdir().expect("tempdir");
        let jsonl = dir.path().join("events.jsonl");
        let trace = dir.path().join("trace.json");
        {
            let telemetry = Telemetry::new("test", Some(&jsonl), Some(&trace)).expect("telemetry");
            let span = telemetry.span("decode", "raw");
            telemetry.counter("queue_depth", "scheduler", 3.0, "tasks");
            span.finish();
        }
        let lines = std::fs::read_to_string(&jsonl).expect("jsonl");
        assert_eq!(lines.lines().count(), 3);
        let trace_text = std::fs::read_to_string(&trace).expect("trace");
        let trace: serde_json::Value = serde_json::from_str(&trace_text).expect("valid trace JSON");
        let phases: Vec<&str> = trace
            .as_array()
            .expect("trace array")
            .iter()
            .map(|event| event["ph"].as_str().expect("trace phase"))
            .collect();
        assert_eq!(phases, ["B", "C", "E"]);
        assert_eq!(trace[1]["args"]["value"], "3");

        let events: Vec<serde_json::Value> = lines
            .lines()
            .map(|line| serde_json::from_str(line).expect("valid JSONL event"))
            .collect();
        let sequences: Vec<u64> = events
            .iter()
            .map(|event| event["sequence"].as_u64().expect("sequence"))
            .collect();
        assert_eq!(sequences, [1, 2, 3]);
    }

    #[test]
    fn slow_live_listener_is_bounded_and_never_backpressures_producer() {
        let telemetry = Telemetry::new("bounded", None, None).expect("telemetry");
        let receiver = telemetry.subscribe();
        for index in 0..1024 {
            telemetry.counter("queue_depth", "scheduler", f64::from(index), "tasks");
        }
        // SyncSender capacity is part of the API's bounded-live-stream
        // contract. The listener is removed once full; durable file output is
        // unaffected (there is no file in this test).
        assert_eq!(receiver.try_iter().count(), 256);
    }
}
