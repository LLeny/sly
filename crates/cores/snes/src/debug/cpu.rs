use super::*;
use crate::cpu::CpuPeek;

pub(super) const MAX_VIEW_FRAMES: usize = 64;

pub(super) fn call_stack_view(
    frames: &[CallFrame],
    pc: u32,
    fmt: impl Fn(u32) -> String,
    symbol: impl Fn(u32) -> String,
) -> CallStackView {
    let mut rows = vec![StackFrame {
        kind: "PC".into(),
        location: fmt(pc).into(),
        symbol: symbol(pc).into(),
        caller: Arc::from(""),
    }];

    for f in frames.iter().rev().take(MAX_VIEW_FRAMES) {
        rows.push(StackFrame {
            kind: f.kind.tag().into(),
            location: fmt(f.target).into(),
            symbol: symbol(f.target).into(),
            caller: format!("from {}", fmt(f.caller)).into(),
        });
    }

    if frames.len() > MAX_VIEW_FRAMES {
        rows.push(StackFrame {
            kind: "…".into(),
            location: Arc::from(""),
            symbol: format!("{} older frames", frames.len() - MAX_VIEW_FRAMES).into(),
            caller: Arc::from(""),
        });
    }

    CallStackView { frames: rows }
}

pub(super) fn cpu_view(cpu: &crate::Cpu) -> CpuView {
    let f = &cpu.flags;
    let acc_bits = if f.m() { 8 } else { 16 };
    let idx_bits = if f.x() { 8 } else { 16 };

    let p = f.raw() as u32;

    CpuView {
        isa: "Ricoh 5A22".into(),
        registers: vec![
            RegisterView::new("A", cpu.acc as u32, acc_bits),
            RegisterView::new("X", cpu.index_x as u32, idx_bits),
            RegisterView::new("Y", cpu.index_y as u32, idx_bits),
            RegisterView::new("S", cpu.stack_pointer as u32, 16),
            RegisterView::new("DB", cpu.data_bank as u32, 8),
            RegisterView::new("DP", cpu.direct_page as u32, 16),
            RegisterView::new("PB", cpu.program_bank as u32, 8),
            RegisterView::new("P", p, 8),
            RegisterView::new("PC", cpu.pc as u32, 16),
        ],
        flags: vec![
            FlagView::new("N", f.n()),
            FlagView::new("V", f.v()),
            FlagView::new("M", f.m()),
            FlagView::new("X", f.x()),
            FlagView::new("D", f.d()),
            FlagView::new("I", f.i()),
            FlagView::new("Z", f.z()),
            FlagView::new("C", f.c()),
        ],
    }
}

pub(super) fn disasm_line_on<B: CpuPeek>(
    cpu: &crate::Cpu,
    bus: &B,
    addr: u16,
    ea: Option<Address>,
) -> (DisasmLine, u8) {
    let bank = cpu.program_bank;
    let d = crate::cpu::disasm::disassemble(cpu, bus, addr);
    let len = d.length.max(1);
    let text = strip_symbol_sigils(&d.text, &SNES_SYNTAX);
    let tokens = tokenize(&text, self::mnemonic_kind(d.class), &SNES_SYNTAX);
    let note = self::disasm_note(&tokens, &d, bank, ea);

    let line = DisasmLine::new(
        addr as u32,
        format!("{bank:02X}:{addr:04X}"),
        self::format_bytes(len, |offset| {
            bus.peek8(Address::new(bank, addr.wrapping_add(offset)))
        }),
        &*text,
        tokens,
    )
    .target(d.target)
    .flow(self::map_flow(d.flow))
    .note(note);

    (line, len)
}

pub(super) fn disasm_line_at(snes: &Snes, addr: u16, with_ea: bool) -> (DisasmLine, u8) {
    let ea = with_ea
        .then(|| crate::cpu::effective_address(snes, addr))
        .flatten();

    self::disasm_line_on(&snes.cpu, &snes.bus, addr, ea)
}

pub(super) fn disasm_view_on<B: CpuPeek>(
    cpu: &crate::Cpu,
    bus: &B,
    base: u16,
    count: usize,
) -> Vec<DisasmLine> {
    let mut addr = base;
    let mut lines = Vec::with_capacity(count);

    for _ in 0..count {
        let (line, len) = self::disasm_line_on(cpu, bus, addr, None);
        lines.push(line);
        addr = addr.wrapping_add(len as u16);
    }

    lines
}

pub(super) fn disasm_view(snes: &Snes, base: u16, count: usize) -> Vec<DisasmLine> {
    self::disasm_view_on(&snes.cpu, &snes.bus, base, count)
}

pub(super) fn fingerprint_65c816<B: CpuPeek>(
    cpu: &crate::Cpu,
    bus: &B,
    base: u16,
    count: usize,
) -> u64 {
    use std::hash::{DefaultHasher, Hasher};

    let mut hasher = DefaultHasher::new();
    let mut addr = base;

    hasher.write_u8(cpu.program_bank);
    hasher.write_usize(crate::cpu::mode_selectors(cpu));
    for _ in 0..count * 4 {
        hasher.write_u8(bus.peek8(Address::new(cpu.program_bank, addr)));
        addr = addr.wrapping_add(1);
    }

    hasher.finish()
}

pub(super) fn spc_cpu_view(spc: &spc700::Spc700) -> CpuView {
    let psw = &spc.psw;
    CpuView {
        isa: "S-SMP".into(),
        registers: vec![
            RegisterView::new("A", spc.a as u32, 8),
            RegisterView::new("X", spc.x as u32, 8),
            RegisterView::new("Y", spc.y as u32, 8),
            RegisterView::new("SP", spc.sp as u32, 8),
            RegisterView::new("PSW", psw.raw() as u32, 8),
            RegisterView::new("PC", spc.pc as u32, 16),
        ],
        flags: vec![
            FlagView::new("N", psw.negative()),
            FlagView::new("V", psw.overflow()),
            FlagView::new("P", psw.direct_page()),
            FlagView::new("B", psw.brk()),
            FlagView::new("H", psw.half_carry()),
            FlagView::new("I", psw.irq_enable()),
            FlagView::new("Z", psw.zero()),
            FlagView::new("C", psw.carry()),
        ],
    }
}

pub(super) fn spc_disasm_view(apu: &Apu, base: u16, count: usize) -> Vec<DisasmLine> {
    let mut addr = base;
    let mut lines = Vec::with_capacity(count);

    for _ in 0..count {
        let (raw, len, target, flow) = spc700::disassemble(apu, addr);
        let len = len.max(1);
        let text = strip_symbol_sigils(&raw, &SPC_SYNTAX);
        let tokens = tokenize(&text, self::spc_mnemonic_kind(&text), &SPC_SYNTAX);
        let note = target.map_or(String::new(), |t| format!("→ ${t:04X}"));

        lines.push(
            DisasmLine::new(
                addr as u32,
                format!("{addr:04X}"),
                self::spc_read_bytes(apu, addr, len),
                &*text,
                tokens,
            )
            .target(target.map(|t| t as u32))
            .flow(self::map_flow(flow))
            .note(note),
        );

        addr = addr.wrapping_add(len as u16);
    }

    lines
}

pub(super) fn spc_read_bytes(apu: &Apu, addr: u16, len: u8) -> String {
    self::format_bytes(len, |offset| apu.peek(addr.wrapping_add(offset)))
}

pub(super) fn spc_mnemonic_kind(text: &str) -> TokenKind {
    let mnemonic = text.split_ascii_whitespace().next().unwrap_or(text);
    match mnemonic {
        "bra" | "beq" | "bne" | "bcs" | "bcc" | "bvs" | "bvc" | "bmi" | "bpl" | "jmp" | "call"
        | "pcall" | "tcall" | "ret" | "reti" | "cbne" | "dbnz" | "bbs" | "bbc" => {
            TokenKind::MnemonicFlow
        }
        "mov" | "movw" | "push" | "pop" => TokenKind::MnemonicMemory,
        "or" | "and" | "eor" | "cmp" | "adc" | "sbc" | "inc" | "dec" | "asl" | "lsr" | "rol"
        | "ror" | "xcn" | "mul" | "div" | "daa" | "das" | "addw" | "subw" | "incw" | "decw"
        | "cmpw" => TokenKind::MnemonicAlu,
        _ => TokenKind::MnemonicSystem,
    }
}

pub(super) const SPC_SYNTAX: SyntaxSpec = SyntaxSpec {
    registers: &["A", "X", "Y", "SP", "YA", "PSW", "C"],
    symbols: crate::apu::is_symbol,
};

pub(super) fn field(label: &str, value: String) -> debugger_model::Row {
    debugger_model::Row::Field(debugger_model::Field {
        label: label.into(),
        value: value.into(),
        accent: false,
    })
}

pub(super) fn flags_row(note: &str, flags: Vec<FlagView>) -> debugger_model::Row {
    debugger_model::Row::Flags {
        flags,
        note: Some((note.into(), debugger_model::Tone::Normal)),
    }
}

pub(super) fn section(title: &str, rows: Vec<debugger_model::Row>) -> debugger_model::Section {
    debugger_model::Section {
        title: Some(title.into()),
        rows,
        ..debugger_model::Section::default()
    }
}

pub(super) fn format_bytes(len: u8, mut byte_at: impl FnMut(u16) -> u8) -> String {
    use std::fmt::Write;

    let mut out = String::with_capacity(len as usize * 3);
    for i in 0..len {
        if i != 0 {
            out.push(' ');
        }
        let _ = write!(out, "{:02X}", byte_at(i as u16));
    }
    out
}

pub(super) fn disasm_note(
    tokens: &[DisasmToken],
    d: &Disasm,
    bank: u8,
    ea: Option<Address>,
) -> String {
    let mut parts: Vec<String> = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Symbol)
        .filter_map(|t| {
            let addr = crate::mmio::address_of(&t.text)?;
            Some(format!("{} = ${addr:04X}", t.text))
        })
        .collect();

    if let Some(ea) = ea {
        let name = crate::mmio::symbol(ea.address()).map_or(String::new(), |n| format!(" = {n}"));
        parts.push(format!("→ {:02X}:{:04X}{name}", ea.bank(), ea.address()));
    } else if parts.is_empty()
        && d.class == InstructionClass::ControlFlow
        && let Some(target) = d.target
    {
        parts.push(format!("→ {bank:02X}:{target:04X}"));
    }

    parts.join("  ")
}

pub(super) fn mnemonic_kind(class: InstructionClass) -> TokenKind {
    match class {
        InstructionClass::ControlFlow => TokenKind::MnemonicFlow,
        InstructionClass::Memory => TokenKind::MnemonicMemory,
        InstructionClass::Arithmetic => TokenKind::MnemonicAlu,
        InstructionClass::Transfer => TokenKind::MnemonicTransfer,
        InstructionClass::System => TokenKind::MnemonicSystem,
        InstructionClass::Other => TokenKind::Mnemonic,
    }
}

pub(super) fn map_flow(flow: CoreFlow) -> FlowKind {
    match flow {
        CoreFlow::None => FlowKind::None,
        CoreFlow::CondBranch => FlowKind::CondBranch,
        CoreFlow::Branch => FlowKind::Branch,
        CoreFlow::Jump => FlowKind::Jump,
        CoreFlow::Call => FlowKind::Call,
        CoreFlow::Return => FlowKind::Return,
    }
}

pub(super) const SNES_SYNTAX: SyntaxSpec = SyntaxSpec {
    registers: &["A", "X", "Y", "S"],
    symbols: crate::mmio::is_symbol,
};

impl SnesDebug {
    pub(super) fn fingerprint(&self, cpu: &CpuId, base: u32, count: usize) -> Option<u64> {
        use std::hash::{DefaultHasher, Hasher};

        let base = base as u16;
        let fingerprint = match cpu.as_str() {
            sa1::CPU => sa1::fingerprint(&self.core, base, count),
            superfx::CPU_GSU => superfx::fingerprint(&self.core, base, count),
            CPU_SPC => {
                let mut hasher = DefaultHasher::new();
                let mut addr = base;
                for _ in 0..count * 3 {
                    hasher.write_u8(self.core.apu.peek(addr));
                    addr = addr.wrapping_add(1);
                }
                hasher.finish()
            }
            _ => self::fingerprint_65c816(&self.core.cpu, &self.core.bus, base, count),
        };

        Some(fingerprint)
    }

    pub(super) fn set_register(&mut self, cpu: &CpuId, name: &str, value: u32) -> bool {
        match cpu.as_str() {
            sa1::CPU => self::set_65c816_register(&mut self.core.sa1.cpu, name, value),
            CPU_SPC => {
                let spc = &mut self.core.apu.spc700;
                match name.to_ascii_uppercase().as_str() {
                    "A" => spc.a = value as u8,
                    "X" => spc.x = value as u8,
                    "Y" => spc.y = value as u8,
                    "SP" => spc.sp = value as u8,
                    "PSW" => spc.psw = spc700::Flags::from_raw(value as u8),
                    "PC" => spc.pc = value as u16,
                    _ => return false,
                }
                true
            }
            superfx::CPU_GSU => {
                if name.eq_ignore_ascii_case("PC") {
                    superfx::set_pc(&mut self.core, value as u16);
                    return true;
                }
                false
            }
            _ => self::set_65c816_register(&mut self.core.cpu, name, value),
        }
    }
}

fn set_65c816_register(cpu: &mut crate::Cpu, name: &str, value: u32) -> bool {
    match name.to_ascii_uppercase().as_str() {
        "A" => cpu.acc = value as u16,
        "X" => cpu.index_x = value as u16,
        "Y" => cpu.index_y = value as u16,
        "S" => cpu.stack_pointer = value as u16,
        "DB" => cpu.data_bank = value as u8,
        "DP" => cpu.direct_page = value as u16,
        "PB" => cpu.program_bank = value as u8,
        "P" => cpu.flags = crate::cpu::Flags::from_raw(value as u8),
        "PC" => cpu.pc = value as u16,
        _ => return false,
    }
    true
}
