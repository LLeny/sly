use logos::Logos;

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
        "read32",
        "sly.read32(addr [, region])",
        "Inspect a little-endian 32-bit value without bus side effects.",
    ),
    builtin(
        "bus_read8",
        "sly.bus_read8(addr [, region])",
        "Execute an 8-bit hardware bus read, including register side effects.",
    ),
    builtin(
        "bus_read16",
        "sly.bus_read16(addr [, region])",
        "Execute a 16-bit hardware bus read.",
    ),
    builtin(
        "bus_read32",
        "sly.bus_read32(addr [, region])",
        "Execute a 32-bit hardware bus read.",
    ),
    builtin(
        "write32",
        "sly.write32(addr, value [, region])",
        "Execute a 32-bit hardware bus write.",
    ),
    builtin(
        "find",
        "sly.find(pattern [, opts])",
        "Search a region for a string or byte table; returns addresses.",
    ),
    builtin(
        "read8",
        "sly.read8(addr [, region])",
        "Inspect one byte without side effects. Region defaults to the primary CPU address space.",
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
        "Execute an 8-bit hardware bus write. Defaults to the primary CPU address space; use sly.write for ROM/RAM patches.",
    ),
    builtin(
        "write16",
        "sly.write16(addr, value [, region])",
        "Execute a 16-bit hardware bus write.",
    ),
    builtin(
        "write",
        "sly.write(addr, bytes [, region])",
        "Patch backing memory without hardware transactions; returns the count changed.",
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
        "Run fn(addr, captured_value, pc, width) on a read overlapping the range.",
    ),
    builtin(
        "on_write",
        "sly.on_write(addr [, last], fn [, opts])",
        "Run fn(addr, captured_value, pc, width) on a write overlapping the range.",
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
