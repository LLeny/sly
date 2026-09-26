mod api;
mod host;
mod lua;
pub use lua::*;

use std::rc::Rc;
use std::sync::Arc;

use debugger_model::{
    BreakpointKind, BreakpointMark, ConsoleLevel, ConsoleLine, DebugCore, EventHit, HookHit,
    ScriptDiagnostic, ScriptStatus,
};
use piccolo::{Closure, Executor, Fuel, Lua, RuntimeError, StashedExecutor, StaticError, Variadic};

pub use host::Trigger;

use crate::host::{Hook, Host};

const FUEL_PER_SLICE: i32 = 16_384;
const CALL_BUDGET: i64 = 32_000_000;
const MAX_HITS_PER_FRAME: usize = 512;

pub struct ScriptEngine {
    lua: Lua,
    executor: StashedExecutor,
    host: Rc<Host>,
    status: ScriptStatus,
    diagnostic: Option<ScriptDiagnostic>,
    marks: Vec<BreakpointMark>,
    event_watches: Vec<Arc<str>>,
}

impl ScriptEngine {
    pub fn new(core: &dyn DebugCore) -> Self {
        let mut lua = Lua::core();
        let host = Rc::new(Host::new(core));

        let executor = lua.enter(|ctx| {
            api::install(ctx, &host);
            ctx.stash(Executor::new(ctx))
        });

        Self {
            lua,
            executor,
            host,
            status: ScriptStatus::Idle,
            diagnostic: None,
            marks: Vec::new(),
            event_watches: Vec::new(),
        }
    }

    pub fn status(&self) -> ScriptStatus {
        self.status
    }

    pub fn is_active(&self) -> bool {
        self.status == ScriptStatus::Active
    }

    pub fn diagnostic(&self) -> Option<&ScriptDiagnostic> {
        self.diagnostic.as_ref()
    }

    pub fn marks(&self) -> &[BreakpointMark] {
        &self.marks
    }

    pub fn hook_count(&self) -> usize {
        self.host.state().hooks.len()
    }

    pub fn take_output(&self) -> Vec<ConsoleLine> {
        self.host.state().output.drain(..).collect()
    }

    pub fn status_line(&self) -> Option<Arc<str>> {
        self.host.state().status.clone()
    }

    pub fn take_pause_request(&self) -> Option<bool> {
        self.host.state().pause.take()
    }

    pub fn run(&mut self, source: &str, core: &mut dyn DebugCore) {
        self.reset(core);

        let Self {
            lua,
            executor,
            host,
            ..
        } = self;

        let outcome = {
            let _guard = host.enter(core);
            lua.try_enter(|ctx| {
                let closure = Closure::load(ctx, Some("script"), source.as_bytes())?;
                ctx.fetch(executor).restart(ctx, closure.into(), ());
                Ok(())
            })
            .and_then(|()| Self::pump(lua, executor))
        };

        match outcome {
            Ok(()) => {
                self.status = ScriptStatus::Active;
                let hooks = self.hook_count();
                self.host.print(
                    ConsoleLevel::Notice,
                    match hooks {
                        0 => "script ran, no hooks".to_string(),
                        1 => "script ran, 1 hook".to_string(),
                        n => format!("script ran, {n} hooks"),
                    },
                );
            }
            Err(err) => self.fail(err),
        }

        self.rebuild_marks();
    }

    pub fn stop(&mut self) {
        let had_hooks = self.disarm();

        self.status = ScriptStatus::Idle;
        self.diagnostic = None;

        if had_hooks > 0 {
            self.host
                .print(ConsoleLevel::Notice, format!("{had_hooks} hooks detached"));
        }
    }

    pub fn dispatch(&mut self, core: &mut dyn DebugCore, hits: &[HookHit]) -> usize {
        if !self.is_active() || hits.is_empty() {
            return 0;
        }

        let mut fired = 0;
        for hit in hits.iter().take(MAX_HITS_PER_FRAME) {
            for id in self.matching(|hook| hook.matches(hit)) {
                if !self.has_hook(id) {
                    continue;
                }

                let args = self.hit_args(hit);
                if !self.call_hook(core, id, &args, None) {
                    self.rebuild_marks();
                    return fired;
                }
                fired += 1;
            }
        }

        self.rebuild_marks();
        fired
    }

    fn matching(&self, wanted: impl Fn(&Hook) -> bool) -> Vec<u64> {
        self.host
            .state()
            .hooks
            .iter()
            .filter(|hook| wanted(hook))
            .map(|hook| hook.id)
            .collect()
    }

    pub fn event_watches(&self) -> &[Arc<str>] {
        &self.event_watches
    }

    pub fn dispatch_events(&mut self, core: &mut dyn DebugCore, events: &[EventHit]) -> usize {
        if !self.is_active() || events.is_empty() {
            return 0;
        }

        let mut fired = 0;
        for event in events.iter().take(MAX_HITS_PER_FRAME) {
            for id in self.matching(|hook| hook.matches_event(event)) {
                if !self.has_hook(id) {
                    continue;
                }
                let extra = (event.detail.clone(), event.pc);
                if !self.call_hook(core, id, &[event.value as i64], Some(extra)) {
                    self.rebuild_marks();
                    return fired;
                }
                fired += 1;
            }
        }

        self.rebuild_marks();
        fired
    }

    pub fn on_frame(&mut self, core: &mut dyn DebugCore) {
        if !self.is_active() {
            return;
        }

        let frame = {
            let mut state = self.host.state();
            state.frame += 1;
            state.frame
        };

        for id in self.matching(|hook| hook.trigger == Trigger::Frame) {
            if !self.has_hook(id) {
                continue;
            }
            if !self.call_hook(core, id, &[frame as i64], None) {
                self.rebuild_marks();
                return;
            }
        }
        self.rebuild_marks();
    }

    fn hit_args(&self, hit: &HookHit) -> Vec<i64> {
        if hit.kind == BreakpointKind::Execute {
            return vec![hit.addr as i64, hit.pc as i64];
        }

        vec![
            hit.addr as i64,
            hit.value as i64,
            hit.pc as i64,
            hit.width.bytes() as i64,
        ]
    }

    fn has_hook(&self, id: u64) -> bool {
        self.host.state().hooks.iter().any(|hook| hook.id == id)
    }

    fn call_hook(
        &mut self,
        core: &mut dyn DebugCore,
        hook_id: u64,
        args: &[i64],
        extra: Option<(Arc<str>, u32)>,
    ) -> bool {
        let Self {
            lua,
            executor,
            host,
            ..
        } = self;

        let outcome = {
            let _guard = host.enter(core);
            lua.try_enter(|ctx| {
                let state = host.state();
                let Some(hook) = state.hooks.iter().find(|hook| hook.id == hook_id) else {
                    return Ok(false);
                };

                let function = ctx.fetch(&hook.func);
                let mut values: Vec<piccolo::Value> = args
                    .iter()
                    .map(|value| piccolo::Value::Integer(*value))
                    .collect();
                if let Some((detail, pc)) = extra.as_ref() {
                    values.push(ctx.intern(detail.as_bytes()).into());
                    values.push(piccolo::Value::Integer(*pc as i64));
                }
                drop(state);

                ctx.fetch(executor).restart(ctx, function, Variadic(values));
                Ok(true)
            })
            .and_then(|started| {
                if started {
                    Self::pump(lua, executor)
                } else {
                    Ok(())
                }
            })
        };

        match outcome {
            Ok(()) => true,
            Err(err) => {
                self.fail(err);
                false
            }
        }
    }

    fn reset(&mut self, core: &dyn DebugCore) {
        self.host.describe(core);

        self.disarm();
        let mut state = self.host.state();
        state.frame = 0;
        drop(state);

        self.status = ScriptStatus::Idle;
        self.diagnostic = None;
    }

    fn pump(lua: &mut Lua, executor: &StashedExecutor) -> Result<(), StaticError> {
        let mut spent: i64 = 0;

        loop {
            let mut fuel = Fuel::with(FUEL_PER_SLICE);
            let done = lua.enter(|ctx| ctx.fetch(executor).step(ctx, &mut fuel));

            spent += (FUEL_PER_SLICE - fuel.remaining()) as i64;

            if done {
                break;
            }

            if spent > CALL_BUDGET {
                lua.enter(|ctx| ctx.fetch(executor).stop(&ctx));
                return Err(RuntimeError::from(anyhow::anyhow!(
                    "script did not yield after {CALL_BUDGET} VM steps, stopped"
                ))
                .into());
            }
        }

        lua.try_enter(|ctx| ctx.fetch(executor).take_result::<()>(ctx)?)
    }

    fn fail(&mut self, err: StaticError) {
        let message = err.to_string();

        self.diagnostic = Some(ScriptDiagnostic {
            line: Self::error_line(&message),
            message: message.as_str().into(),
        });
        self.status = ScriptStatus::Failed;
        self.host.print(ConsoleLevel::Error, message);

        self.disarm();
    }

    fn error_line(message: &str) -> Option<usize> {
        let tail = message.split("line ").nth(1)?;
        let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    }

    fn disarm(&mut self) -> usize {
        let count = {
            let mut state = self.host.state();
            state.status = None;
            state.pause = None;
            let count = state.hooks.len();
            state.hooks.clear();
            count
        };
        self.marks.clear();
        self.event_watches.clear();
        count
    }

    fn rebuild_marks(&mut self) {
        let state = self.host.state();

        self.marks = state.hooks.iter().filter_map(|hook| hook.mark()).collect();
        self.event_watches = state
            .hooks
            .iter()
            .filter_map(|hook| match &hook.trigger {
                Trigger::Event { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        drop(state);

        self.event_watches.sort();
        self.event_watches.dedup();
    }
}
