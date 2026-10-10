//! In-memory capture of tracing events, so the web UI can show what the
//! library is doing while an action runs.

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

/// How many lines the buffer keeps.
pub const MAX_LINES: usize = 5000;
/// Default page size for the /api/logs endpoint.
pub const DEFAULT_LIMIT: usize = 500;

#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    pub seq: u64,
    pub level: String,
    pub target: String,
    pub message: String,
}

#[derive(Debug)]
pub struct LogBuffer {
    seq: AtomicU64,
    lines: Mutex<VecDeque<LogLine>>,
}

impl LogBuffer {
    fn new() -> Self {
        Self {
            seq: AtomicU64::new(0),
            lines: Mutex::new(VecDeque::new()),
        }
    }

    fn push(&self, level: &str, target: &str, message: String) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let mut lines = lock(&self.lines);

        lines.push_back(LogLine {
            seq,
            level: level.to_string(),
            target: target.to_string(),
            message,
        });

        while lines.len() > MAX_LINES {
            lines.pop_front();
        }
    }

    /// Every line newer than the given sequence number, oldest first.
    pub fn snapshot(&self, since: u64, limit: usize) -> Vec<LogLine> {
        let lines = lock(&self.lines);
        lines
            .iter()
            .filter(|line| line.seq > since)
            .take(limit.clamp(1, MAX_LINES))
            .cloned()
            .collect()
    }

    /// Sequence number of the newest line.
    pub fn last_seq(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    pub fn clear(&self) {
        lock(&self.lines).clear();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        // a poisoned lock only means some log line was lost, keep serving
        Err(poisoned) => poisoned.into_inner(),
    }
}

static BUFFER: OnceLock<LogBuffer> = OnceLock::new();

/// The process wide log buffer, created on first use so the CLI and the web UI
/// always talk to the same one.
pub fn buffer() -> &'static LogBuffer {
    BUFFER.get_or_init(LogBuffer::new)
}

/// Tracing layer feeding the log buffer. Install it with
/// tracing_subscriber::registry().with(WebLogLayer::new()).
#[derive(Debug, Default)]
pub struct WebLogLayer;

impl WebLogLayer {
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for WebLogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        let metadata = event.metadata();
        buffer().push(
            metadata.level().as_str(),
            metadata.target(),
            visitor.finish(),
        );
    }
}

#[derive(Debug, Default)]
struct MessageVisitor {
    message: Option<String>,
    fields: Vec<String>,
}

impl MessageVisitor {
    fn finish(&self) -> String {
        match (&self.message, self.fields.is_empty()) {
            (Some(message), true) => message.clone(),
            (Some(message), false) => format!("{} ({})", message, self.fields.join(", ")),
            (None, false) => self.fields.join(", "),
            (None, true) => String::new(),
        }
    }
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let rendered = format!("{:?}", value);
        if field.name() == "message" {
            self.message = Some(rendered);
        } else {
            self.fields.push(format!("{}={}", field.name(), rendered));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.push(format!("{}={}", field.name(), value));
        }
    }
}
