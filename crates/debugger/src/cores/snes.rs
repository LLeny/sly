use cirsium::Highlight;
use cirsium::components::StatusView;
use cirsium::widgets::{
    BreakpointKind, CartridgeField, CartridgeView, MemoryRegion, MmioFieldView, MmioGroup,
    MmioRegisterView, MmioView,
};
use debugger_model::{
    CpuView, DisasmLine, DisasmToken, FlagView, FlowKind, RegisterView, SyntaxSpec, TokenKind,
    strip_symbol_sigils, tokenize,
};
use gpui::SharedString;
use snes::apu::{Apu, spc700};
use snes::cartridge::{MapMode, Speed};
use snes::cpu::disasm::{Disasm, FlowKind as CoreFlow, InstructionClass};
use snes::mmio::{Address, MemKind, MemoryAccess};
use snes::{Cartridge, Cpu, Snes};

use crate::art::{AssetProvider, GameRef, System};
use crate::breakpoints::BreakpointMark;
use crate::processor::{CpuDescriptor, CpuId};
use crate::target::{DebugTarget, RunOutcome};

const WRAM_SIZE: u32 = 0x2_0000;
const PPU_BASE: u32 = 0x2100;
const PPU_SIZE: u32 = 0x0100;
const CPU_BASE: u32 = 0x4200;
const CPU_SIZE: u32 = 0x0200;
const BUS_SIZE: u32 = 0x100_0000;

const CPU_MAIN: &str = "cpu.65c816";
const CPU_SPC: &str = "apu.spc700";

pub struct SnesTarget {
    core: Snes,
    cartridge: CartridgeView,
    system: SharedString,
    label: SharedString,
    segments: Vec<SharedString>,
    last_access: MemoryAccess,
    fault: Option<&'static str>,
}

impl SnesTarget {
    pub fn load(
        path: &str,
        provider: &dyn AssetProvider,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let rom = std::fs::read(path)?;
        let title = crate::db::title(System::Snes, &rom);
        let cart = Cartridge::new(rom)?;

        let file_name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string());

        let name: SharedString = title.map(SharedString::from).unwrap_or_else(|| {
            std::path::Path::new(path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| file_name.clone())
                .into()
        });

        let mut cartridge = cartridge_view(&cart);
        cartridge.cover = provider.cover_art(&GameRef::new(System::Snes, name));

        let segments = vec![
            mapper_name(&cart).into(),
            cart.region().tv_system().to_string().into(),
            human_size(cart.rom.len()).into(),
        ];

        Ok(Self {
            core: Snes::new(cart),
            cartridge,
            system: "SNES".into(),
            label: file_name.into(),
            segments,
            last_access: MemoryAccess::default(),
            fault: None,
        })
    }

    fn region_byte(&self, region: &str, addr: u32) -> u8 {
        match region {
            "wram" => self.core.mmu.wram.get(addr as usize).copied().unwrap_or(0),
            "rom" => self
                .core
                .mmu
                .cartridge
                .rom
                .get(addr as usize)
                .copied()
                .unwrap_or(0),
            "bus" => self.core.peek8(Address::from_24bit(addr & 0x00FF_FFFF)),
            "spc" => self.core.apu.peek(addr as u16),
            _ => self.core.peek8(Address::new(0x00, addr as u16)),
        }
    }

    fn region_key(&self, region: &str, addr: u32) -> Option<(MemKind, u32)> {
        match region {
            "wram" => Some((MemKind::Wram, addr)),
            "rom" => Some((MemKind::Rom, addr)),
            "bus" => self.core.classify(Address::from_24bit(addr & 0x00FF_FFFF)),
            // TODO: MemKind
            "spc" => None,
            _ => Some((MemKind::Io, addr)),
        }
    }

    fn catch(&mut self, f: impl FnOnce(&mut Snes)) -> bool {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut self.core))).is_ok()
    }

    fn step_machine(&mut self) -> (MemoryAccess, Option<&'static str>) {
        let mut access = None;

        let ok = self.catch(|core| access = Some(core.trace(snes::cpu::step)));
        let access = access.unwrap_or_default();

        if !ok {
            return (access, Some(CPU_MAIN));
        }

        if !self.catch(|core| spc700::step(&mut core.apu)) {
            return (access, Some(CPU_SPC));
        }

        (access, None)
    }

    fn execute_hit(&self, breakpoints: &[BreakpointMark]) -> bool {
        let pcs = [
            (CPU_MAIN, self.core.cpu.pc as u32),
            (CPU_SPC, self.core.apu.spc700.pc as u32),
        ];

        breakpoints
            .iter()
            .filter(|m| m.kind == BreakpointKind::Execute)
            .any(|m| {
                pcs.iter().any(|(id, pc)| {
                    m.cpu.as_str() == *id && (m.start & 0xFFFF..=m.end & 0xFFFF).contains(pc)
                })
            })
    }

    fn watch_hit(&self, breakpoints: &[BreakpointMark], access: &MemoryAccess) -> bool {
        breakpoints.iter().any(|m| {
            let set = match m.kind {
                BreakpointKind::Read => &access.reads,
                BreakpointKind::Write => &access.writes,
                _ => return false,
            };
            match m.region.as_ref() {
                "bus" => (m.start..=m.end).any(|addr| {
                    self.region_key("bus", addr)
                        .is_some_and(|key| set.contains(&key))
                }),
                _ => set.iter().any(|&key| {
                    (m.start..=m.end).contains(&key.1)
                        && self.region_key(&m.region, key.1) == Some(key)
                }),
            }
        })
    }
}

type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>;

struct SilencedPanics(Option<PanicHook>);

impl SilencedPanics {
    fn new() -> Self {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        Self(Some(prev))
    }
}

impl Drop for SilencedPanics {
    fn drop(&mut self) {
        if let Some(prev) = self.0.take() {
            std::panic::set_hook(prev);
        }
    }
}

fn merge_access(total: &mut MemoryAccess, step: &MemoryAccess) {
    total.reads.extend(step.reads.iter().copied());
    total.writes.extend(step.writes.iter().copied());
    if step.last.is_some() {
        total.last = step.last;
    }
}

impl DebugTarget for SnesTarget {
    fn cpus(&self) -> Vec<CpuDescriptor> {
        vec![
            CpuDescriptor::new(CPU_MAIN, "65C816", "bus"),
            CpuDescriptor::new(CPU_SPC, "SPC700", "spc"),
        ]
    }

    fn pc(&self, cpu: &CpuId) -> u32 {
        match cpu.as_str() {
            CPU_SPC => self.core.apu.spc700.pc as u32,
            _ => self.core.cpu.pc as u32,
        }
    }

    fn set_pc(&mut self, cpu: &CpuId, addr: u32) {
        self.fault = None;
        match cpu.as_str() {
            CPU_SPC => self.core.apu.spc700.pc = addr as u16,
            _ => self.core.cpu.pc = addr as u16,
        }
    }

    fn step(&mut self) -> bool {
        let _silenced = SilencedPanics::new();
        let (access, fault) = self.step_machine();
        self.last_access = access;
        self.fault = fault;
        fault.is_none()
    }

    fn run_frame(&mut self, breakpoints: &[BreakpointMark]) -> RunOutcome {
        let watch = breakpoints
            .iter()
            .any(|m| matches!(m.kind, BreakpointKind::Read | BreakpointKind::Write));

        let _silenced = SilencedPanics::new();
        let mut access = MemoryAccess::default();

        for _ in 0..snes::INSTRUCTIONS_PER_FRAME {
            let (step, fault) = self.step_machine();
            merge_access(&mut access, &step);

            if let Some(cpu) = fault {
                self.last_access = access;
                self.fault = Some(cpu);
                return RunOutcome::Fault;
            }

            if !breakpoints.is_empty()
                && ((watch && self.watch_hit(breakpoints, &step)) || self.execute_hit(breakpoints))
            {
                self.last_access = access;
                return RunOutcome::Breakpoint;
            }
        }

        self.last_access = access;
        self.fault = None;
        RunOutcome::Done
    }

    fn reset(&mut self) {
        self.core.cpu = Cpu::new(self.core.read16(Address::new(0x00, 0xFFFC)));
        self.last_access = MemoryAccess::default();
        self.fault = None;
    }

    fn cpu_view(&self, cpu: &CpuId) -> CpuView {
        match cpu.as_str() {
            CPU_SPC => spc_cpu_view(&self.core.apu.spc700),
            _ => cpu_view(&self.core.cpu),
        }
    }

    fn disassemble(&self, cpu: &CpuId, base: u32, count: usize) -> Vec<DisasmLine> {
        match cpu.as_str() {
            CPU_SPC => spc_disasm_view(&self.core.apu, base as u16, count),
            _ => disasm_view(&self.core, base as u16, count),
        }
    }

    fn cartridge(&self) -> &CartridgeView {
        &self.cartridge
    }

    fn mmio_view(&self) -> MmioView {
        mmio_view(&self.core)
    }

    fn status(&self, label: &str) -> StatusView {
        StatusView {
            system: self.system.clone(),
            session: self.label.clone(),
            segments: self.segments.clone(),
            state: caption(label, &self.core),
            ..Default::default()
        }
    }

    fn caption(&self, label: &str) -> SharedString {
        if self.fault == Some(CPU_SPC) {
            return format!("{label} @ SPC700:{:04X}", self.core.apu.spc700.pc).into();
        }
        caption(label, &self.core)
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion::new("wram", "WRAM", 0, WRAM_SIZE),
            MemoryRegion::new("rom", "ROM", 0, self.core.mmu.cartridge.rom.len() as u32),
            MemoryRegion::new("ppu", "PPU/APU", PPU_BASE, PPU_SIZE),
            MemoryRegion::new("cpu", "CPU/DMA", CPU_BASE, CPU_SIZE),
            MemoryRegion::new("bus", "CPU Bus", 0, BUS_SIZE),
            MemoryRegion::new("spc", "SPC RAM", 0, 0x1_0000),
        ]
    }

    fn read_memory(&self, region: &str, addr: u32, len: usize) -> Vec<u8> {
        (0..len as u32)
            .map(|i| self.region_byte(region, addr.wrapping_add(i)))
            .collect()
    }

    fn format_address(&self, region: &str, addr: u32) -> SharedString {
        match region {
            "wram" => format!("{:02X}:{:04X}", 0x7E + (addr >> 16), addr & 0xFFFF),
            "rom" => format!("{addr:06X}"),
            "bus" => format!("{:02X}:{:04X}", (addr >> 16) & 0xFF, addr & 0xFFFF),
            "spc" => format!("{:04X}", addr & 0xFFFF),
            _ => format!("00:{:04X}", addr & 0xFFFF),
        }
        .into()
    }

    fn memory_highlights(&self, region: &str, addr: u32, len: usize) -> Vec<Highlight> {
        (0..len as u32)
            .map(|i| match self.region_key(region, addr.wrapping_add(i)) {
                Some(key) if self.last_access.writes.contains(&key) => Highlight::Written,
                Some(key) if self.last_access.reads.contains(&key) => Highlight::Read,
                _ => Highlight::None,
            })
            .collect()
    }

    fn annotate(&self, addr: u32) -> SharedString {
        snes::mmio::symbol((addr & 0xFFFF) as u16)
            .map(SharedString::from)
            .unwrap_or_default()
    }

    fn latest_access(&self) -> Option<(SharedString, u32)> {
        let (kind, offset) = self.last_access.last?;
        let region = match kind {
            MemKind::Wram => "wram",
            MemKind::Rom => "rom",
            MemKind::Io if (PPU_BASE..PPU_BASE + PPU_SIZE).contains(&offset) => "ppu",
            MemKind::Io if (CPU_BASE..CPU_BASE + CPU_SIZE).contains(&offset) => "cpu",
            MemKind::Io => "bus",
        };
        Some((region.into(), offset))
    }
}

fn caption(label: &str, snes: &Snes) -> SharedString {
    format!(
        "{label} @ {:02X}:{:04X}",
        snes.cpu.program_bank, snes.cpu.pc
    )
    .into()
}

fn cpu_view(cpu: &snes::Cpu) -> CpuView {
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

fn disasm_view(snes: &Snes, base: u16, count: usize) -> Vec<DisasmLine> {
    let bank = snes.cpu.program_bank;
    let pc = snes.cpu.pc;
    let mut addr = base;
    let mut lines = Vec::with_capacity(count);

    for _ in 0..count {
        let d = snes.disassemble_at(addr);
        let len = d.length.max(1);
        let text = strip_symbol_sigils(&d.text, &SNES_SYNTAX);
        let tokens = tokenize(&text, mnemonic_kind(d.class), &SNES_SYNTAX);
        let ea = (addr == pc)
            .then(|| effective_address(snes, &text))
            .flatten();
        let note = disasm_note(&tokens, &d, bank, ea);

        lines.push(
            DisasmLine::new(
                addr as u32,
                format!("{bank:02X}:{addr:04X}"),
                read_bytes(snes, bank, addr, len),
                &*text,
                tokens,
                addr == pc,
            )
            .target(d.target)
            .flow(map_flow(d.flow))
            .note(note),
        );

        addr = addr.wrapping_add(len as u16);
    }

    lines
}

fn spc_cpu_view(spc: &spc700::Spc700) -> CpuView {
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

fn spc_disasm_view(apu: &Apu, base: u16, count: usize) -> Vec<DisasmLine> {
    let pc = apu.spc700.pc;
    let mut addr = base;
    let mut lines = Vec::with_capacity(count);

    for _ in 0..count {
        let (raw, len, target, flow) = spc700::disassemble(apu, addr);
        let len = len.max(1);
        let text = strip_symbol_sigils(&raw, &SPC_SYNTAX);
        let tokens = tokenize(&text, spc_mnemonic_kind(&text), &SPC_SYNTAX);
        let note = target.map_or(String::new(), |t| format!("→ ${t:04X}"));

        lines.push(
            DisasmLine::new(
                addr as u32,
                format!("{addr:04X}"),
                spc_read_bytes(apu, addr, len),
                &*text,
                tokens,
                addr == pc,
            )
            .target(target.map(|t| t as u32))
            .flow(map_flow(flow))
            .note(note),
        );

        addr = addr.wrapping_add(len as u16);
    }

    lines
}

fn spc_read_bytes(apu: &Apu, addr: u16, len: u8) -> String {
    format_bytes(len, |offset| apu.peek(addr.wrapping_add(offset)))
}

fn spc_mnemonic_kind(text: &str) -> TokenKind {
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

const SPC_SYNTAX: SyntaxSpec = SyntaxSpec {
    registers: &["A", "X", "Y", "SP", "YA", "PSW", "C"],
    symbols: snes::apu::is_symbol,
};

fn read_bytes(snes: &Snes, bank: u8, addr: u16, len: u8) -> String {
    format_bytes(len, |offset| {
        snes.peek8(Address::new(bank, addr.wrapping_add(offset)))
    })
}

fn format_bytes(len: u8, mut byte_at: impl FnMut(u16) -> u8) -> String {
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

fn disasm_note(tokens: &[DisasmToken], d: &Disasm, bank: u8, ea: Option<Address>) -> String {
    let mut parts: Vec<String> = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Symbol)
        .filter_map(|t| {
            let addr = snes::mmio::address_of(&t.text)?;
            Some(format!("{} = ${addr:04X}", t.text))
        })
        .collect();

    if let Some(ea) = ea {
        let name = snes::mmio::symbol(ea.address()).map_or(String::new(), |n| format!(" = {n}"));
        parts.push(format!("→ {:02X}:{:04X}{name}", ea.bank(), ea.address()));
    } else if parts.is_empty()
        && d.class == InstructionClass::ControlFlow
        && let Some(target) = d.target
    {
        parts.push(format!("→ {bank:02X}:{target:04X}"));
    }

    parts.join("  ")
}

fn effective_address(snes: &Snes, text: &str) -> Option<Address> {
    use snes::cpu::addressing::AddrMode::*;

    let op = text.split_once(' ')?.1.trim();
    let (base, width) = operand_base(op)?;
    let (dp, ab) = (base as u8, base as u16);

    let mode = match width {
        2 if op.ends_with(",s),y") => Isy(dp),
        2 if op.ends_with(",x)") => Idx(dp),
        2 if op.ends_with("),y") => Idy(dp),
        2 if op.ends_with("],y") => Idly(dp),
        2 if op.ends_with(",s") => Sr(dp),
        2 if op.ends_with(']') => Idl(dp),
        2 if op.ends_with(')') => Idp(dp),
        2 if op.ends_with(",x") => DpX(dp),
        2 if op.ends_with(",y") => DpY(dp),
        4 if op.ends_with(",x") => AbsX(ab),
        4 if op.ends_with(",y") => AbsY(ab),
        6 if op.ends_with(",x") => AlX(base),
        _ => return None,
    };

    Some(snes::cpu::addressing::resolve(snes, mode))
}

fn operand_base(op: &str) -> Option<(u32, usize)> {
    if let Some((_, rest)) = op.split_once('$') {
        let hex: String = rest.chars().take_while(char::is_ascii_hexdigit).collect();
        return Some((u32::from_str_radix(&hex, 16).ok()?, hex.len()));
    }

    op.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .find_map(|tok| Some((snes::mmio::address_of(tok)? as u32, 4)))
}

fn mnemonic_kind(class: InstructionClass) -> TokenKind {
    match class {
        InstructionClass::ControlFlow => TokenKind::MnemonicFlow,
        InstructionClass::Memory => TokenKind::MnemonicMemory,
        InstructionClass::Arithmetic => TokenKind::MnemonicAlu,
        InstructionClass::Transfer => TokenKind::MnemonicTransfer,
        InstructionClass::System => TokenKind::MnemonicSystem,
        InstructionClass::Other => TokenKind::Mnemonic,
    }
}

fn map_flow(flow: CoreFlow) -> FlowKind {
    match flow {
        CoreFlow::None => FlowKind::None,
        CoreFlow::CondBranch => FlowKind::CondBranch,
        CoreFlow::Branch => FlowKind::Branch,
        CoreFlow::Jump => FlowKind::Jump,
        CoreFlow::Call => FlowKind::Call,
        CoreFlow::Return => FlowKind::Return,
    }
}

const SNES_SYNTAX: SyntaxSpec = SyntaxSpec {
    registers: &["A", "X", "Y", "S"],
    symbols: snes::mmio::is_symbol,
};

fn mmio_component(addr: u16) -> (u8, &'static str) {
    match addr {
        0x2100..=0x213F => (0, "PPU"),
        0x2140..=0x2143 => (1, "APU"),
        0x420B..=0x420C | 0x4300..=0x43FF => (2, "DMA"),
        0x4200..=0x42FF => (3, "CPU"),
        _ => (9, "Other"),
    }
}

fn read_mmio(core: &Snes, addr: u16, bits: u8) -> u32 {
    let lo = core.peek_mmio(addr) as u32;
    if bits > 8 {
        lo | (core.peek_mmio(addr.wrapping_add(1)) as u32) << 8
    } else {
        lo
    }
}

fn mmio_field_views(fields: &[snes::mmio::FieldInfo], parent: u32) -> Vec<MmioFieldView> {
    let mut ordered: Vec<_> = fields.iter().collect();
    ordered.sort_by_key(|f| f.offset);
    ordered
        .into_iter()
        .map(|f| mmio_field_view(f, parent))
        .collect()
}

fn mmio_field_view(field: &snes::mmio::FieldInfo, parent: u32) -> MmioFieldView {
    use snes::mmio::FieldKind;

    let mask = if field.width >= 32 {
        u32::MAX
    } else {
        (1u32 << field.width) - 1
    };
    let value = (parent >> field.offset) & mask;

    let bit_label = if field.width == 1 {
        format!("[{}]", field.offset)
    } else {
        format!("[{}:{}]", field.offset, field.offset + field.width - 1)
    };

    let (display, is_bool, children) = match field.kind {
        FieldKind::Bool => (
            if value != 0 { "on" } else { "off" }.to_string(),
            true,
            Vec::new(),
        ),
        FieldKind::Uint => (format!("${value:X}"), false, Vec::new()),
        FieldKind::Enum(info) => {
            let display = info
                .variants
                .iter()
                .find(|(disc, _)| *disc == value as u128)
                .map(|(_, name)| format!("{name} ({value})"))
                .unwrap_or_else(|| format!("${value:X}"));
            (display, false, Vec::new())
        }
        FieldKind::Struct(fields) => (
            format!("${value:X}"),
            false,
            mmio_field_views(fields, value),
        ),
    };

    MmioFieldView {
        name: field.name.into(),
        bit_label: bit_label.into(),
        value,
        display: display.into(),
        is_bool,
        children,
    }
}

fn mmio_view(core: &Snes) -> MmioView {
    let mut symbols: Vec<_> = snes::mmio::symbols().collect();
    symbols.sort_by_key(|s| (mmio_component(s.addr).0, s.addr));

    let mut groups: Vec<MmioGroup> = Vec::new();
    for sym in symbols {
        let (_, component) = mmio_component(sym.addr);
        let value = read_mmio(core, sym.addr, sym.bits);
        let fields = mmio_field_views(sym.fields, value);

        let reg = MmioRegisterView {
            id: format!("mmio-{:04X}", sym.addr).into(),
            name: sym.name.into(),
            addr: format!("${:04X}", sym.addr).into(),
            value,
            bits: sym.bits,
            highlight: Highlight::None,
            fields,
        };

        match groups.last_mut() {
            Some(group) if group.name == component => group.registers.push(reg),
            _ => groups.push(MmioGroup {
                name: component.into(),
                registers: vec![reg],
            }),
        }
    }

    MmioView { groups }
}

fn cartridge_view(cart: &Cartridge) -> CartridgeView {
    let speed = match cart.memory_map() {
        Ok(mm) => match mm.speed() {
            Speed::Fast => "Fast (3.58 MHz)",
            Speed::Slow => "Slow (2.68 MHz)",
        },
        Err(_) => "unknown",
    };

    CartridgeView {
        title: cart.title().into(),
        fields: vec![
            CartridgeField::new("Mapper", mapper_name(cart)).accented(),
            CartridgeField::new("Speed", speed),
            CartridgeField::new("Region", cart.region().to_string()),
            CartridgeField::new("ROM size", human_size(cart.rom.len())),
            CartridgeField::new("Header", format!("${:06X}", cart.header_offset())),
        ],
        cover: None,
    }
}

fn mapper_name(cart: &Cartridge) -> &'static str {
    match cart.memory_map() {
        Ok(mm) => match mm.map_mode() {
            MapMode::LoRom => "LoROM",
            MapMode::HiRom => "HiROM",
            MapMode::Sdd1 => "S-DD1",
            MapMode::Sa1 => "SA-1",
            MapMode::ExHiRom => "ExHiROM",
            MapMode::Spc7110 => "SPC7110",
        },
        Err(_) => "unknown",
    }
}

fn human_size(bytes: usize) -> String {
    const MB: usize = 1 << 20;
    const KB: usize = 1 << 10;

    if bytes >= MB && bytes.is_multiple_of(MB) {
        format!("{} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{bytes} B")
    }
}
