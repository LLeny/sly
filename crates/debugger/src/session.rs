use cirsium::components::StatusView;
use cirsium::widgets::{
    CartridgeField, CartridgeView, CpuView, DisasmLine, DisasmToken, DisassemblyView, FlagView,
    RegisterView, TokenKind,
};
use gpui::SharedString;
use snes::cartridge::{MapMode, Speed};
use snes::{Cartridge, Cpu, Snes};

use crate::art::{AssetProvider, GameRef, System};

pub struct Session {
    core: Snes,
    pub cartridge: CartridgeView,
    system: SharedString,
    label: SharedString,
    segments: Vec<SharedString>,
}

impl Session {
    pub fn load(
        path: &str,
        provider: &dyn AssetProvider,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let rom = std::fs::read(path)?;
        let cart = Cartridge::new(rom)?;

        let file_name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string());
        let rom_stem = std::path::Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| file_name.clone());

        let mut cartridge = cartridge_view(&cart);
        cartridge.cover = provider.cover_art(&GameRef::new(System::Snes, rom_stem));

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

    pub fn pc(&self) -> u16 {
        self.core.cpu.pc
    }

    pub fn step(&mut self) -> bool {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.core.step()));
        std::panic::set_hook(prev);
        result.is_ok()
    }

    pub fn reset(&mut self) {
        self.core.cpu = Cpu::new(self.core.read16(0x00, 0xFFFC));
    }

    pub fn set_pc(&mut self, addr: u16) {
        self.core.cpu.pc = addr;
    }

    pub fn cpu_view(&self) -> CpuView {
        cpu_view(&self.core.cpu)
    }

    pub fn disassemble(&self, base: u16, count: usize) -> DisassemblyView {
        disasm_view(&self.core, base, count)
    }

    pub fn status(&self, label: &str) -> StatusView {
        StatusView {
            system: self.system.clone(),
            session: self.label.clone(),
            segments: self.segments.clone(),
            state: caption(label, &self.core),
            ..Default::default()
        }
    }

    pub fn caption(&self, label: &str) -> SharedString {
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
        let tokens = tokenize(&d.text);
        lines.push(DisasmLine::new(
            addr as u32,
            format!("{bank:02X}:{addr:04X}"),
            read_bytes(snes, bank, addr, len),
            d.text,
            tokens,
            addr == pc,
        ));
        addr = addr.wrapping_add(len as u16);
    }

    DisassemblyView {
        lines,
        show_bytes: false,
    }
}

fn read_bytes(snes: &Snes, bank: u8, addr: u16, len: u8) -> String {
    (0..len)
        .map(|i| format!("{:02X}", snes.read8(bank, addr.wrapping_add(i as u16))))
        .collect::<Vec<_>>()
        .join(" ")
}

fn tokenize(text: &str) -> Vec<DisasmToken> {
    let b = text.as_bytes();
    let n = b.len();
    let mut i = 0;
    let mut tokens = Vec::new();

    let start = i;
    while i < n && b[i].is_ascii_alphabetic() {
        i += 1;
    }

    if i > start {
        tokens.push(DisasmToken::new(&text[start..i], TokenKind::Mnemonic));
    }

    while i < n && b[i] == b' ' {
        i += 1;
    }

    while i < n {
        let c = b[i];
        if c == b'#' || c == b'$' {
            let s = i;
            i += 1;
            if c == b'#' && i < n && b[i] == b'$' {
                i += 1;
            }
            while i < n && b[i].is_ascii_hexdigit() {
                i += 1;
            }
            tokens.push(DisasmToken::new(&text[s..i], TokenKind::Number));
        } else if c.is_ascii_alphabetic() {
            let s = i;
            while i < n && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = &text[s..i];
            let kind = match word {
                "A" | "X" | "Y" | "S" => TokenKind::Register,
                _ => TokenKind::Plain,
            };
            tokens.push(DisasmToken::new(word, kind));
        } else if matches!(c, b',' | b'(' | b')' | b'[' | b']') {
            tokens.push(DisasmToken::new(&text[i..i + 1], TokenKind::Punctuation));
            i += 1;
        } else {
            let s = i;
            i += 1;
            while i < n && b[i] == c {
                i += 1;
            }
            tokens.push(DisasmToken::new(&text[s..i], TokenKind::Plain));
        }
    }

    tokens
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
