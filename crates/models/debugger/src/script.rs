use std::sync::Arc;

use crate::{AccessSource, AccessWidth};

use crate::target::{BreakpointKind, CpuId};

pub const SCRIPT_WORKSPACE: &str = "script";

pub const SCRIPT_EDITOR_PANEL: &str = "script.editor";
pub const SCRIPT_CONSOLE_PANEL: &str = "script.console";
pub const SCRIPT_BUILTINS_PANEL: &str = "script.builtins";

#[derive(Clone)]
pub struct HookHit {
    pub kind: BreakpointKind,
    pub cpu: CpuId,
    pub region: Arc<str>,
    pub addr: u32,
    pub value: u32,
    pub width: AccessWidth,
    pub source: AccessSource,
    /// For execute hits this is the next instruction.
    pub pc: u32,
}

#[derive(Clone)]
pub struct EventDescriptor {
    pub id: Arc<str>,
    pub label: Arc<str>,
}

impl EventDescriptor {
    pub fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

#[derive(Clone)]
pub struct EventHit {
    pub id: Arc<str>,
    pub value: u32,
    pub detail: Arc<str>,
    pub pc: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ConsoleLevel {
    #[default]
    Print,
    Notice,
    Warn,
    Error,
}

#[derive(Clone)]
pub struct ConsoleLine {
    pub level: ConsoleLevel,
    pub text: Arc<str>,
}

impl ConsoleLine {
    pub fn new(level: ConsoleLevel, text: impl Into<Arc<str>>) -> Self {
        Self {
            level,
            text: text.into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ScriptStatus {
    #[default]
    Idle,
    Active,
    Failed,
}

impl ScriptStatus {
    pub fn label(self) -> &'static str {
        match self {
            ScriptStatus::Idle => "idle",
            ScriptStatus::Active => "active",
            ScriptStatus::Failed => "error",
        }
    }
}

#[derive(Clone, Default)]
pub struct ScriptDiagnostic {
    pub message: Arc<str>,
    pub line: Option<usize>,
}
