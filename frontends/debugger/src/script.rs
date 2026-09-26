use std::sync::Arc;

use cirsium::components::Completion;
use cirsium::widgets::{ConsoleEntry, ScriptConsoleView};
use debugger_model::{BreakpointMark, ConsoleLevel, ConsoleLine, DebugCore, HookHit, ScriptStatus};
use gpui::{SharedString, UniformListScrollHandle};
use scripting::ScriptEngine;

const CONSOLE_LIMIT: usize = 2000;

pub(crate) struct Script {
    engine: ScriptEngine,
    console: Arc<ScriptConsoleView>,
    scroll: UniformListScrollHandle,
    ran_source: Option<String>,
}

impl Script {
    pub(crate) fn new(target: &dyn DebugCore) -> Self {
        Self {
            engine: ScriptEngine::new(target),
            console: Arc::default(),
            scroll: UniformListScrollHandle::new(),
            ran_source: None,
        }
    }

    pub(crate) fn console(&self) -> Arc<ScriptConsoleView> {
        self.console.clone()
    }

    pub(crate) fn scroll(&self) -> UniformListScrollHandle {
        self.scroll.clone()
    }

    pub(crate) fn status(&self) -> ScriptStatus {
        self.engine.status()
    }

    pub(crate) fn is_active(&self) -> bool {
        self.engine.is_active()
    }

    pub(crate) fn hook_count(&self) -> usize {
        self.engine.hook_count()
    }

    pub(crate) fn status_line(&self) -> Option<SharedString> {
        self.engine.status_line().map(SharedString::from)
    }

    pub(crate) fn error(&self) -> Option<SharedString> {
        self.engine
            .diagnostic()
            .map(|diagnostic| SharedString::from(diagnostic.message.clone()))
    }

    pub(crate) fn error_line(&self) -> Option<usize> {
        self.engine.diagnostic().and_then(|d| d.line)
    }

    pub(crate) fn is_dirty(&self, source: &str) -> bool {
        self.ran_source.as_deref() != Some(source)
    }

    pub(crate) fn marks(&self) -> &[BreakpointMark] {
        self.engine.marks()
    }

    pub(crate) fn run(&mut self, source: &str, target: &mut dyn DebugCore) {
        self.engine.run(source, target);
        self.ran_source = Some(source.to_string());
        self.drain();
    }

    pub(crate) fn stop(&mut self) {
        self.engine.stop();
        self.ran_source = None;
        self.drain();
    }

    pub(crate) fn clear_console(&mut self) {
        self.console = Arc::default();
    }

    pub(crate) fn dispatch(&mut self, target: &mut dyn DebugCore, hits: &[HookHit]) -> usize {
        let fired = self.engine.dispatch(target, hits);
        self.drain();
        fired
    }

    pub(crate) fn dispatch_events(
        &mut self,
        target: &mut dyn DebugCore,
        events: &[debugger_model::EventHit],
    ) -> usize {
        let fired = self.engine.dispatch_events(target, events);
        self.drain();
        fired
    }

    pub(crate) fn event_watches(&self) -> &[Arc<str>] {
        self.engine.event_watches()
    }

    pub(crate) fn on_frame(&mut self, target: &mut dyn DebugCore) {
        self.engine.on_frame(target);
        self.drain();
    }

    pub(crate) fn take_pause_request(&self) -> Option<bool> {
        self.engine.take_pause_request()
    }

    pub(crate) fn note(&mut self, text: impl Into<Arc<str>>) {
        self.append(vec![ConsoleLine::new(ConsoleLevel::Notice, text)]);
    }

    fn drain(&mut self) {
        self.append(self.engine.take_output());
    }

    fn append(&mut self, lines: Vec<ConsoleLine>) {
        if lines.is_empty() {
            return;
        }

        let view = Arc::make_mut(&mut self.console);
        view.entries
            .extend(lines.into_iter().map(ConsoleEntry::from));

        let overflow = view.entries.len().saturating_sub(CONSOLE_LIMIT);
        if overflow > 0 {
            view.entries.drain(..overflow);
        }

        let last = view.entries.len().saturating_sub(1);
        self.scroll.scroll_to_item(last, gpui::ScrollStrategy::Top);
    }
}

pub(crate) fn default_script(system: core_model::System) -> &'static str {
    match system {
        core_model::System::Snes => include_str!("../../../lua/snes/default.lua"),
    }
}

pub(crate) fn api_completions() -> Vec<Completion> {
    scripting::BUILTIN_GROUPS
        .iter()
        .flat_map(|group| group.entries)
        .map(|builtin| {
            let name = format!("sly.{}", builtin.name);
            Completion::new(name.clone(), builtin.signature, format!("{name}("))
                .doc(builtin.summary)
        })
        .collect()
}
