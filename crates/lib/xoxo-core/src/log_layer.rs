//! `tracing` layer that fans log events out over the bus.
//!
//! Compiled only with the `log-broadcast` feature. Consumers that want
//! diagnostics delivered as [`crate::bus::LogRecord`] messages (for instance a
//! TUI log pane) install this alongside — or instead of — the default stdout
//! formatter.
//!
//! Design note: `tracing::Layer` is the idiomatic "transport" abstraction for
//! this ecosystem, so we do not invent a parallel trait. Swapping or composing
//! sinks happens through the existing `.with(layer)` builder.

use tokio::sync::broadcast;
use tracing::{Event, Subscriber, field::Visit};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

use crate::bus::{LogLevel, LogRecord};

/// Tracing layer that converts each event into a [`LogRecord`] and publishes
/// it on a broadcast channel.
pub struct BusLogLayer {
    tx: broadcast::Sender<LogRecord>,
}

impl BusLogLayer {
    /// Build a layer backed by the given broadcast sender. The sender is
    /// typically obtained from [`crate::bus::Bus::logs_sender`].
    pub fn new(tx: broadcast::Sender<LogRecord>) -> Self {
        Self { tx }
    }
}

impl<S> Layer<S> for BusLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();

        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        let record = LogRecord {
            level: level_of(*meta.level()),
            target: meta.target().to_string(),
            message: visitor.message,
        };

        // No subscribers is a normal state (e.g. daemon started before the TUI
        // attached). Drop the record rather than treating it as an error.
        let _ = self.tx.send(record);
    }
}

fn level_of(level: tracing::Level) -> LogLevel {
    match level {
        tracing::Level::TRACE => LogLevel::Trace,
        tracing::Level::DEBUG => LogLevel::Debug,
        tracing::Level::INFO => LogLevel::Info,
        tracing::Level::WARN => LogLevel::Warn,
        tracing::Level::ERROR => LogLevel::Error,
    }
}

/// Captures the `message` field of a tracing event. Other fields are ignored
/// for now — structured fields can be surfaced later without breaking the
/// payload, since [`LogRecord`] owns its shape.
#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            use std::fmt::Write as _;
            // `write!` into a `String` never fails; ignore the Result to keep
            // the visitor infallible.
            let _ = write!(&mut self.message, "{value:?}");
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::Bus;
    use tracing::info;
    use tracing_subscriber::layer::SubscriberExt;

    #[tokio::test]
    async fn publishes_tracing_events_onto_the_bus() {
        let (bus, _inbox) = Bus::new();
        let mut logs = bus.subscribe_logs();

        let layer = BusLogLayer::new(bus.logs_sender());
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            info!(target: "xoxo::test", "hello {}", "world");
        });

        let rec = logs.recv().await.expect("record received");
        assert_eq!(rec.level, LogLevel::Info);
        assert_eq!(rec.target, "xoxo::test");
        assert_eq!(rec.message, "hello world");
    }
}
