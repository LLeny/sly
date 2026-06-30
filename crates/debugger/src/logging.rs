use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use cirsium::components::ComboOption;
use cirsium::widgets::{LogEntry, LogLevel, LogView, short_target};
use gpui::{Pixels, Point, SharedString, UniformListScrollHandle};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

const CAPACITY: usize = 2000;
const CORE_CRATES: &[&str] = &["snes"];

#[derive(Clone)]
pub struct LogBuffer {
    inner: Arc<Mutex<VecDeque<LogEntry>>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(CAPACITY))),
        }
    }

    fn push(&self, entry: LogEntry) {
        let mut entries = self.inner.lock().unwrap();
        while entries.len() >= CAPACITY {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    fn clear(&self) {
        self.inner.lock().unwrap().clear();
    }

    fn with_entries<R>(&self, f: impl FnOnce(&VecDeque<LogEntry>) -> R) -> R {
        f(&self.inner.lock().unwrap())
    }
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LogLayer {
    buffer: LogBuffer,
}

impl LogLayer {
    pub fn new(buffer: LogBuffer) -> Self {
        Self { buffer }
    }
}

impl<S: Subscriber> Layer<S> for LogLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        self.buffer.push(LogEntry {
            level: level_view(*metadata.level()),
            target: metadata.target().to_string().into(),
            message: visitor.message.into(),
        });
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if !self.message.is_empty() {
            self.message.push(' ');
        }

        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.message, "{}={:?}", field.name(), value);
        }
    }
}

#[derive(Clone)]
enum TargetFilter {
    All,
    Cores,
    Exact(SharedString),
}

impl TargetFilter {
    fn matches(&self, target: &str) -> bool {
        match self {
            TargetFilter::All => true,
            TargetFilter::Cores => is_core_target(target),
            TargetFilter::Exact(wanted) => wanted.as_ref() == target,
        }
    }

    fn id(&self) -> SharedString {
        match self {
            TargetFilter::All => "all".into(),
            TargetFilter::Cores => "cores".into(),
            TargetFilter::Exact(target) => target.clone(),
        }
    }

    fn label(&self) -> SharedString {
        match self {
            TargetFilter::All => "All".into(),
            TargetFilter::Cores => "Cores".into(),
            TargetFilter::Exact(target) => short_target(target).into(),
        }
    }
}

pub struct Logs {
    pub scroll: UniformListScrollHandle,
    pub target_combo: Option<Point<Pixels>>,
    pub level_combo: Option<Point<Pixels>>,
    buffer: LogBuffer,
    target: TargetFilter,
    level: Option<LogLevel>,
    message: String,
}

impl Logs {
    pub fn new(buffer: LogBuffer) -> Self {
        Self {
            scroll: UniformListScrollHandle::new(),
            target_combo: None,
            level_combo: None,
            buffer,
            target: TargetFilter::Cores,
            level: None,
            message: String::new(),
        }
    }

    pub fn view(&self) -> LogView {
        let needle = self.message.trim().to_lowercase();
        let min_rank = self.level.map(LogLevel::rank);
        let target = &self.target;
        let entries = self.buffer.with_entries(|entries| {
            entries
                .iter()
                .filter(|entry| {
                    target.matches(&entry.target)
                        && min_rank.is_none_or(|rank| entry.level.rank() >= rank)
                        && (needle.is_empty() || entry.message.to_lowercase().contains(&needle))
                })
                .cloned()
                .collect::<Vec<LogEntry>>()
        });

        LogView { entries }
    }

    pub fn target_options(&self) -> Vec<ComboOption> {
        let mut targets: Vec<SharedString> = self
            .buffer
            .with_entries(|entries| entries.iter().map(|entry| entry.target.clone()).collect());

        targets.sort_by(|a, b| a.as_ref().cmp(b.as_ref()));
        targets.dedup();

        let mut options = vec![
            ComboOption::new("all", "All"),
            ComboOption::new("cores", "Cores"),
        ];
        options.extend(targets.into_iter().map(|target| {
            let label = short_target(&target);
            ComboOption::new(target, label)
        }));

        options
    }

    pub fn level_options(&self) -> Vec<ComboOption> {
        let mut options = vec![ComboOption::new("all", "All")];
        options.extend(
            LogLevel::ALL
                .iter()
                .map(|level| ComboOption::new(level.id(), level.label())),
        );
        options
    }

    pub fn target_id(&self) -> SharedString {
        self.target.id()
    }

    pub fn level_id(&self) -> SharedString {
        self.level.map(|l| l.id().into()).unwrap_or("all".into())
    }

    pub fn target_label(&self) -> SharedString {
        self.target.label()
    }

    pub fn level_label(&self) -> SharedString {
        self.level.map(|l| l.label().into()).unwrap_or("All".into())
    }

    pub fn set_target(&mut self, id: &str) {
        self.target = match id {
            "all" => TargetFilter::All,
            "cores" => TargetFilter::Cores,
            other => TargetFilter::Exact(other.to_string().into()),
        };
        self.target_combo = None;
    }

    pub fn set_level(&mut self, id: &str) {
        self.level = LogLevel::from_id(id);
        self.level_combo = None;
    }

    pub fn set_message(&mut self, text: String) {
        self.message = text;
    }

    pub fn open_target_combo(&mut self, position: Point<Pixels>) {
        self.target_combo = Some(position);
        self.level_combo = None;
    }

    pub fn close_target_combo(&mut self) -> bool {
        self.target_combo.take().is_some()
    }

    pub fn open_level_combo(&mut self, position: Point<Pixels>) {
        self.level_combo = Some(position);
        self.target_combo = None;
    }

    pub fn close_level_combo(&mut self) -> bool {
        self.level_combo.take().is_some()
    }

    pub fn follow_tail(&self) {
        if self.scroll.is_scrolled_to_end().unwrap_or(true) {
            self.scroll.scroll_to_bottom();
        }
    }

    pub fn clear(&self) {
        self.buffer.clear();
    }
}

fn level_view(level: Level) -> LogLevel {
    match level {
        Level::ERROR => LogLevel::Error,
        Level::WARN => LogLevel::Warn,
        Level::INFO => LogLevel::Info,
        Level::DEBUG => LogLevel::Debug,
        Level::TRACE => LogLevel::Trace,
    }
}

fn is_core_target(target: &str) -> bool {
    let crate_name = target.split("::").next().unwrap_or(target);
    CORE_CRATES.contains(&crate_name)
}
