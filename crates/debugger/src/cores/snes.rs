use cirsium::components::StatusView;
use cirsium::widgets::{
    CartridgeField, CartridgeView, CpuView, DisasmLine, DisasmToken, DisassemblyView, FlagView,
    RegisterView, TokenKind,
};
use gpui::SharedString;
use snes::cartridge::{MapMode, Speed};
use snes::cpu::disasm::{Disasm, InstructionClass};
use snes::mmio::Address;
use snes::{Cartridge, Cpu, Snes};

use crate::art::{AssetProvider, GameRef, System};
use crate::cores::disasm::{SyntaxSpec, strip_symbol_sigils, tokenize};
use crate::target::DebugTarget;

pub struct SnesTarget {
    core: Snes,
    cartridge: CartridgeView,
    system: SharedString,
    label: SharedString,
    segments: Vec<SharedString>,
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
        })
    }

    fn guarded(&mut self, f: impl FnOnce(&mut Snes)) -> bool {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut self.core)));
        std::panic::set_hook(prev);
        result.is_ok()
    }
}

impl DebugTarget for SnesTarget {
    fn pc(&self) -> u32 {
        self.core.cpu.pc as u32
    }

    fn set_pc(&mut self, addr: u32) {
        self.core.cpu.pc = addr as u16;
    }

    fn step(&mut self) -> bool {
        self.guarded(|core| core.step())
    }

    fn run_frame(&mut self) -> bool {
        self.guarded(|core| core.run_frame())
    }

    fn reset(&mut self) {
        self.core.cpu = Cpu::new(self.core.read16(Address::new(0x00, 0xFFFC)));
    }

    fn cpu_view(&self) -> CpuView {
        cpu_view(&self.core.cpu)
    }

    fn disassemble(&self, base: u32, count: usize) -> DisassemblyView {
        disasm_view(&self.core, base as u16, count)
    }

    fn cartridge(&self) -> &CartridgeView {
        &self.cartridge
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
        caption(label, &self.core)
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

    let p = (f.c() as u32)
        | (f.z() as u32) << 1
        | (f.i() as u32) << 2
        | (f.d() as u32) << 3
        | (f.x() as u32) << 4
        | (f.m() as u32) << 5
        | (f.v() as u32) << 6
        | (f.n() as u32) << 7;

    CpuView {
        isa: "65C816".into(),
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

fn disasm_view(snes: &Snes, base: u16, count: usize) -> DisassemblyView {
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
                text,
                tokens,
                addr == pc,
            )
            .target(d.target)
            .note(note),
        );

        addr = addr.wrapping_add(len as u16);
    }

    DisassemblyView {
        lines,
        show_bytes: false,
    }
}

fn read_bytes(snes: &Snes, bank: u8, addr: u16, len: u8) -> String {
    (0..len)
        .map(|i| {
            format!(
                "{:02X}",
                snes.read8(Address::new(bank, addr.wrapping_add(i as u16)))
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
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
    } else if parts.is_empty() && d.class == InstructionClass::ControlFlow {
        if let Some(target) = d.target {
            parts.push(format!("→ {bank:02X}:{target:04X}"));
        }
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

const SNES_SYNTAX: SyntaxSpec = SyntaxSpec {
    registers: &["A", "X", "Y", "S"],
    symbols: snes::mmio::is_symbol,
};

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
