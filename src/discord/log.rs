use super::redact::log_line;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::{Layer, Registry};

const MAX_BYTES: u64 = 64 * 1024;
const TARGET: &str = concat!(env!("CARGO_CRATE_NAME"), "::discord");

struct TextLayer<F>(F);

#[derive(Default)]
struct Message(String);

impl Visit for Message {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.0, "{value:?}");
        }
    }
}

impl<S: Subscriber, F: Fn(&str) + Send + Sync + 'static> Layer<S> for TextLayer<F> {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let mut message = Message::default();
        event.record(&mut message);
        (self.0)(&log_line(&message.0));
    }
}

fn subscriber(sink: impl Fn(&str) + Send + Sync + 'static) -> impl Subscriber {
    Registry::default().with(
        TextLayer(sink).with_filter(filter_fn(|metadata| metadata.target().starts_with(TARGET))),
    )
}

pub(crate) fn init() {
    let _ = tracing::subscriber::set_global_default(subscriber(append));
}

fn append(line: &str) {
    let Some(base) = std::env::var_os("APPDATA") else {
        return;
    };
    let path = PathBuf::from(base).join("EclipticaHUD").join("discord.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let _ = writeln!(file, "{secs} {line}");
    }
}

#[cfg(test)]
thread_local! {
    static LOGGED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(super) fn take_logged() -> Vec<String> {
    static CAPTURE: std::sync::Once = std::sync::Once::new();
    CAPTURE.call_once(|| {
        let _ = tracing::subscriber::set_global_default(subscriber(|line| {
            LOGGED.with(|logged| logged.borrow_mut().push(line.to_string()));
        }));
    });
    LOGGED.with(|logged| std::mem::take(&mut *logged.borrow_mut()))
}
