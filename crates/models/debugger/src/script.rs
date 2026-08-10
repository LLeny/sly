use std::sync::Arc;

use logos::Logos;

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

#[derive(Clone, Copy)]
pub struct Builtin {
    pub name: &'static str,
    pub signature: &'static str,
    pub summary: &'static str,
}

#[derive(Clone, Copy)]
pub struct BuiltinGroup {
    pub name: &'static str,
    pub entries: &'static [Builtin],
}

const fn builtin(name: &'static str, signature: &'static str, summary: &'static str) -> Builtin {
    Builtin {
        name,
        signature,
        summary,
    }
}

pub const MEMORY_BUILTINS: &[Builtin] = &[
    builtin(
        "find",
        "sly.find(pattern [, opts])",
        "Search a region for a string or byte table; returns addresses.",
    ),
    builtin(
        "read8",
        "sly.read8(addr [, region])",
        "Read one byte. Region defaults to the active CPU bus.",
    ),
    builtin(
        "read16",
        "sly.read16(addr [, region])",
        "Read a little-endian 16-bit word.",
    ),
    builtin(
        "read24",
        "sly.read24(addr [, region])",
        "Read a little-endian 24-bit value.",
    ),
    builtin(
        "read",
        "sly.read(addr, len [, region])",
        "Read len bytes into a 1-based table.",
    ),
    builtin(
        "write8",
        "sly.write8(addr, value [, region])",
        "Write one byte.",
    ),
    builtin(
        "write16",
        "sly.write16(addr, value [, region])",
        "Write a little-endian 16-bit word.",
    ),
    builtin(
        "write",
        "sly.write(addr, bytes [, region])",
        "Write a table of bytes; returns the count written.",
    ),
    builtin(
        "regions",
        "sly.regions()",
        "List the memory regions this core exposes.",
    ),
];

pub const CPU_BUILTINS: &[Builtin] = &[
    builtin("cpus", "sly.cpus()", "List the processor ids of this core."),
    builtin("pc", "sly.pc([cpu])", "Current program counter."),
    builtin(
        "setpc",
        "sly.setpc(addr [, cpu])",
        "Move the program counter.",
    ),
    builtin(
        "reg",
        "sly.reg(name [, cpu])",
        "Read a register by name, e.g. sly.reg(\"A\").",
    ),
    builtin(
        "setreg",
        "sly.setreg(name, value [, cpu])",
        "Write a register; returns true when the core accepted it.",
    ),
    builtin(
        "registers",
        "sly.registers([cpu])",
        "All registers as a name to value table.",
    ),
    builtin(
        "flags",
        "sly.flags([cpu])",
        "All CPU flags as a name to boolean table.",
    ),
];

pub const HOOK_BUILTINS: &[Builtin] = &[
    builtin(
        "on_event",
        "sly.on_event(id [, value], fn)",
        "Run fn(value, detail, pc) on a core event, e.g. \"scanline\".",
    ),
    builtin(
        "events",
        "sly.events()",
        "List the event ids this core can report.",
    ),
    builtin(
        "on_exec",
        "sly.on_exec(addr [, last], fn [, opts])",
        "Run fn(addr, pc) when the CPU executes in the range.",
    ),
    builtin(
        "on_read",
        "sly.on_read(addr [, last], fn [, opts])",
        "Run fn(addr, value, pc) on a read in the range.",
    ),
    builtin(
        "on_write",
        "sly.on_write(addr [, last], fn [, opts])",
        "Run fn(addr, value, pc) on a write in the range.",
    ),
    builtin(
        "on_frame",
        "sly.on_frame(fn)",
        "Run fn(frame) once per emulated frame.",
    ),
    builtin(
        "remove",
        "sly.remove(id)",
        "Detach a hook by the id its registration returned.",
    ),
    builtin(
        "clear_hooks",
        "sly.clear_hooks()",
        "Detach every hook this script registered.",
    ),
];

pub const CONTROL_BUILTINS: &[Builtin] = &[
    builtin(
        "pause",
        "sly.pause()",
        "Pause emulation after the callback.",
    ),
    builtin("resume", "sly.resume()", "Resume emulation."),
    builtin(
        "button",
        "sly.button(name, pressed [, player])",
        "Hold or release a controller button.",
    ),
    builtin(
        "frame",
        "sly.frame()",
        "Frames emulated since the script started.",
    ),
    builtin(
        "status",
        "sly.status(text)",
        "Set the one-line status shown above the editor.",
    ),
    builtin("log", "sly.log(...)", "Print a line to the script console."),
    builtin("warn", "sly.warn(...)", "Print a warning line."),
    builtin("error", "sly.error(...)", "Print an error line."),
    builtin(
        "hex",
        "sly.hex(value [, digits])",
        "Format a value as $-prefixed uppercase hex.",
    ),
    builtin(
        "dump",
        "sly.dump(value)",
        "Render any value, tables included, as readable text.",
    ),
];

pub const BUILTIN_GROUPS: &[BuiltinGroup] = &[
    BuiltinGroup {
        name: "Memory",
        entries: MEMORY_BUILTINS,
    },
    BuiltinGroup {
        name: "CPU",
        entries: CPU_BUILTINS,
    },
    BuiltinGroup {
        name: "Hooks",
        entries: HOOK_BUILTINS,
    },
    BuiltinGroup {
        name: "Control",
        entries: CONTROL_BUILTINS,
    },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LuaTokenKind {
    Keyword,
    Builtin,
    Ident,
    Number,
    Str,
    Comment,
    Operator,
    Punctuation,
    Plain,
}

#[derive(Clone, Copy, Debug)]
pub struct LuaToken {
    pub start: usize,
    pub end: usize,
    pub kind: LuaTokenKind,
}

pub const LUA_KEYWORDS: [&str; 22] = [
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

const GLOBALS: [&str; 17] = [
    "sly",
    "print",
    "pairs",
    "ipairs",
    "next",
    "select",
    "tonumber",
    "tostring",
    "type",
    "pcall",
    "error",
    "assert",
    "string",
    "table",
    "math",
    "os",
    "coroutine",
];

#[derive(Logos)]
#[logos(skip r"[ \t\r\n]+")]
enum Lexeme {
    #[regex(r"--\[\[(?:[^\]]|\][^\]])*(?:\]\])?")]
    #[regex(r"--[^\n]*", allow_greedy = true)]
    Comment,

    #[regex(r"\[\[(?:[^\]]|\][^\]])*(?:\]\])?")]
    #[regex(r#""(?:[^"\\\n]|\\.)*"?"#)]
    #[regex(r"'(?:[^'\\\n]|\\.)*'?")]
    Str,

    #[regex(r"0[xX][0-9a-fA-F]+")]
    #[regex(r"[0-9]+(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?")]
    Number,

    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Ident,

    #[regex(r"[+\-*/%^#=<>~]+")]
    Operator,

    #[regex(r"[(){}\[\];:,.]")]
    Punctuation,
}

pub fn tokenize_lua(source: &str) -> Vec<LuaToken> {
    let mut tokens = Vec::new();
    let mut lexer = Lexeme::lexer(source);

    while let Some(result) = lexer.next() {
        let span = lexer.span();
        let kind = match result {
            Ok(Lexeme::Comment) => LuaTokenKind::Comment,
            Ok(Lexeme::Str) => LuaTokenKind::Str,
            Ok(Lexeme::Number) => LuaTokenKind::Number,
            Ok(Lexeme::Operator) => LuaTokenKind::Operator,
            Ok(Lexeme::Punctuation) => LuaTokenKind::Punctuation,
            Ok(Lexeme::Ident) => {
                let text = &source[span.clone()];
                if LUA_KEYWORDS.contains(&text) {
                    LuaTokenKind::Keyword
                } else if GLOBALS.contains(&text) {
                    LuaTokenKind::Builtin
                } else {
                    LuaTokenKind::Ident
                }
            }
            Err(()) => LuaTokenKind::Plain,
        };

        tokens.push(LuaToken {
            start: span.start,
            end: span.end,
            kind,
        });
    }

    tokens
}
