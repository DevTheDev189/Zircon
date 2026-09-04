//! In-memory ring buffer of recent `tracing` events, exposed to the Settings
//! tab as launcher debug logs.
//!
//! The buffer keeps the last [`MAX_LOG_LINES`] formatted lines. This is purely
//! diagnostic: it never touches disk, so nothing sensitive is persisted, and it
//! is cleared explicitly via `clear_launcher_logs` (or on app restart).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

const MAX_LOG_LINES: usize = 2000;

static LOG_BUFFER: OnceLock<Arc<Mutex<VecDeque<String>>>> = OnceLock::new();

/// Shared handle to the ring buffer of formatted log lines.
pub fn log_buffer() -> Arc<Mutex<VecDeque<String>>> {
    LOG_BUFFER
        .get_or_init(|| Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_LINES))))
        .clone()
}

/// A [`Layer`] that records every event into the shared ring buffer.
pub struct InMemoryLogLayer;

impl<S: Subscriber> Layer<S> for InMemoryLogLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = LogVisitor(String::new());
        event.record(&mut visitor);
        let metadata = event.metadata();
        let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
        let line = format!(
            "[{timestamp}] [{level}] [{target}] {msg}",
            level = metadata.level(),
            target = metadata.target(),
            msg = visitor.0
        );

        let buffer = log_buffer();
        {
            let mut guard = match buffer.lock() {
                Ok(guard) => guard,
                Err(_) => return, // Buffer poisoned: nothing to record.
            };
            if guard.len() >= MAX_LOG_LINES {
                guard.pop_front();
            }
            guard.push_back(line);
        }
    }
}

/// Collects the event's fields into a single display string (`message` first,
/// then `key=value` pairs).
struct LogVisitor(String);

impl tracing::field::Visit for LogVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        } else {
            if !self.0.is_empty() {
                self.0.push(' ');
            }
            self.0.push_str(&format!("{}={:?}", field.name(), value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0 = value.to_string();
        } else {
            if !self.0.is_empty() {
                self.0.push(' ');
            }
            self.0.push_str(&format!("{}={}", field.name(), value));
        }
    }
}
