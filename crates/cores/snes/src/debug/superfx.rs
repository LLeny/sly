use std::cell::Cell;

use debugger_model::{
    Cell as TableCell, ColWidth, Column, CpuDescriptor, CpuView, DisasmLine, Field, FlagView,
    FlowKind, RasterImage, RegisterView, Row, Section, StructuredView, SyntaxSpec, TableRow,
    TableView, TokenKind, Tone, hex, strip_symbol_sigils, tokenize,
};

use super::{field, format_bytes, section};
use crate::Snes;
use crate::mmio::{Address, AddressSpace};
use crate::superfx::{SuperFx, bitmap, decoder, map};

pub const CPU_GSU: &str = "cart.gsu";
pub const WORKSPACE: &str = "superfx";
pub const STATE_ID: &str = "gsu_state";
pub const CACHE_ID: &str = "gsu_cache";

pub const GSU_SYNTAX: SyntaxSpec = SyntaxSpec {
    registers: &[
        "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "r13",
        "r14", "r15",
    ],
    symbols: |_| false,
};

pub fn descriptor() -> CpuDescriptor {
    CpuDescriptor::new(CPU_GSU, "GSU", "gsu")
}

pub fn bus_peek(snes: &Snes, bank: u8, addr: u16) -> u8 {
    match map::gsu_space(&snes.mmu.cartridge, Address::new(bank, addr)) {
        AddressSpace::Cart(region, index) => snes.mmu.cart_read(region, index),
        _ => 0,
    }
}

pub fn pc(fx: &SuperFx) -> u16 {
    fx.pipeline_addr
}

pub fn set_pc(snes: &mut Snes, addr: u16) {
    let pbr = snes.superfx.pbr;
    snes.superfx.pipeline = self::bus_peek(snes, pbr, addr);
    snes.superfx.pipeline_addr = addr;
    snes.superfx.r[15] = addr.wrapping_add(1);
}

pub fn cpu_view(fx: &SuperFx) -> CpuView {
    let mut registers: Vec<RegisterView> = (0..16)
        .map(|i| RegisterView::new(format!("R{i}"), fx.r[i] as u32, 16))
        .collect();
    registers.push(RegisterView::new("SFR", fx.sfr.raw() as u32, 16));
    registers.push(RegisterView::new("PBR", fx.pbr as u32, 8));
    registers.push(RegisterView::new("CBR", fx.cbr as u32, 16));

    CpuView {
        isa: "GSU".into(),
        registers,
        flags: vec![
            FlagView::new("GO", fx.sfr.go()),
            FlagView::new("R", fx.sfr.rom_busy()),
            FlagView::new("IRQ", fx.sfr.irq()),
            FlagView::new("A1", fx.sfr.alt1()),
            FlagView::new("A2", fx.sfr.alt2()),
            FlagView::new("B", fx.sfr.with()),
            FlagView::new("OV", fx.sfr.overflow()),
            FlagView::new("S", fx.sfr.sign()),
            FlagView::new("C", fx.sfr.carry()),
            FlagView::new("Z", fx.sfr.zero()),
        ],
    }
}

struct Reader<'a> {
    snes: &'a Snes,
    bank: u8,
    modes: Cell<(bool, bool, bool)>,
}

impl decoder::DisasmCtx for Reader<'_> {
    fn read_u8(&self, addr: u64) -> u8 {
        self::bus_peek(self.snes, self.bank, addr as u16)
    }

    fn symbol(&self, _addr: u64) -> Option<(String, u64)> {
        None
    }

    fn mode(&self, name: &str) -> u64 {
        let (alt1, alt2, b) = self.modes.get();
        match name {
            "alt1" => alt1 as u64,
            "alt2" => alt2 as u64,
            "b" => b as u64,
            _ => 0,
        }
    }
}

fn mnemonic(op: u8, (alt1, alt2, b): (bool, bool, bool)) -> (decoder::Mnemonic, usize) {
    let combo = decoder::pack_modes(alt1 as u64, alt2 as u64, b as u64);
    let id = decoder::Instruction(op).opcode_in(combo);
    (decoder::OPCODE_MNEMONIC[id], id)
}

fn advance_modes(m: decoder::Mnemonic, (alt1, alt2, b): (bool, bool, bool)) -> (bool, bool, bool) {
    use decoder::Mnemonic as M;

    match m {
        M::Alt1 => (true, alt2, false),
        M::Alt2 => (alt1, true, false),
        M::Alt3 => (true, true, false),
        M::With => (alt1, alt2, true),
        M::To | M::From => (alt1, alt2, b),
        _ => (false, false, false),
    }
}

fn flow_kind(m: decoder::Mnemonic, id: usize) -> FlowKind {
    use decoder::Mnemonic as M;

    if decoder::OPCODE_TAGS[id].contains(&"jump") {
        return FlowKind::Jump;
    }
    match m {
        M::Stop => FlowKind::Return,
        M::Bra => FlowKind::Branch,
        M::Bge
        | M::Blt
        | M::Bne
        | M::Beq
        | M::Bpl
        | M::Bmi
        | M::Bcc
        | M::Bcs
        | M::Bvc
        | M::Bvs
        | M::Loop => FlowKind::Branch,
        _ => FlowKind::None,
    }
}

fn mnemonic_kind(m: decoder::Mnemonic, id: usize) -> TokenKind {
    use decoder::Mnemonic as M;

    match m {
        M::Stop
        | M::Nop
        | M::Cache
        | M::Alt1
        | M::Alt2
        | M::Alt3
        | M::Color
        | M::Cmode
        | M::Getc
        | M::Ramb
        | M::Romb => TokenKind::MnemonicSystem,
        M::To | M::From | M::With | M::Move | M::Moves => TokenKind::MnemonicTransfer,
        M::Stw
        | M::Stb
        | M::Ldw
        | M::Ldb
        | M::Sbk
        | M::Lm
        | M::Sm
        | M::Lms
        | M::Sms
        | M::Ibt
        | M::Iwt
        | M::Getb
        | M::Getbh
        | M::Getbl
        | M::Getbs => TokenKind::MnemonicMemory,
        m if self::flow_kind(m, id) != FlowKind::None => TokenKind::MnemonicFlow,
        _ => TokenKind::MnemonicAlu,
    }
}

pub fn disasm_view(snes: &Snes, base: u32, count: usize) -> Vec<DisasmLine> {
    let fx = &snes.superfx;
    let bank = (base >> 16) as u8;
    let reader = Reader {
        snes,
        bank,
        modes: Cell::new((fx.sfr.alt1(), fx.sfr.alt2(), fx.sfr.with())),
    };

    let mut addr = base as u16;
    let mut lines = Vec::with_capacity(count);

    for _ in 0..count {
        let op = self::bus_peek(snes, bank, addr);
        let (m, id) = self::mnemonic(op, reader.modes.get());

        let (text, length) = decoder::disasm_ctx(addr as u64, &reader);
        let length = length.max(1);
        let text = strip_symbol_sigils(&text, &GSU_SYNTAX);
        let tokens = tokenize(&text, self::mnemonic_kind(m, id), &GSU_SYNTAX);

        let flow = self::flow_kind(m, id);
        let target = match op {
            0x05..=0x0F => {
                let disp = self::bus_peek(snes, bank, addr.wrapping_add(1)) as i8;
                Some(addr.wrapping_add(2).wrapping_add_signed(disp as i16) as u32)
            }
            _ => None,
        };

        let note = target.map_or(String::new(), |t| format!("→ {:02X}:{t:04X}", bank));

        lines.push(
            DisasmLine::new(
                Address::new(bank, addr).as_24bit(),
                length,
                format!("{:02X}:{addr:04X}", bank),
                self::read_bytes(snes, bank, addr, length),
                &*text,
                tokens,
            )
            .target(target.map(|target| (u32::from(bank) << 16) | target))
            .flow(flow)
            .conditional(matches!(op, 0x06..=0x0F | 0x3C))
            .note(note),
        );

        reader.modes.set(self::advance_modes(m, reader.modes.get()));
        addr = addr.wrapping_add(length as u16);
    }

    lines
}

fn read_bytes(snes: &Snes, bank: u8, addr: u16, len: u8) -> String {
    format_bytes(len, |offset| {
        self::bus_peek(snes, bank, addr.wrapping_add(offset))
    })
}

pub fn fingerprint(snes: &Snes, base: u32, count: usize) -> u64 {
    use std::hash::{DefaultHasher, Hasher};

    let fx = &snes.superfx;
    let bank = (base >> 16) as u8;
    let mut hasher = DefaultHasher::new();
    hasher.write_u8(bank);
    hasher.write_u16(fx.sfr.raw());

    let mut addr = base as u16;
    for _ in 0..count * 3 {
        hasher.write_u8(self::bus_peek(snes, bank, addr));
        addr = addr.wrapping_add(1);
    }

    hasher.finish()
}

pub fn state_view(snes: &Snes) -> StructuredView {
    let fx = &snes.superfx;

    let clock = if fx.clsr.fast_clock() {
        "21.4 MHz"
    } else {
        "10.7 MHz"
    };
    let status = Section {
        leading: true,
        ..section(
            "Status",
            vec![Row::Flags {
                flags: vec![
                    FlagView::new("GO", fx.sfr.go()),
                    FlagView::new("IRQ", fx.sfr.irq()),
                    FlagView::new("W-ROM", fx.wait_rom),
                    FlagView::new("W-RAM", fx.wait_ram),
                ],
                note: Some((
                    format!(
                        "{clock}{}",
                        if fx.cfgr.fast_multiply() { "  MS0" } else { "" }
                    )
                    .into(),
                    Tone::Normal,
                )),
            }],
        )
    };

    let banks = section(
        "Banks",
        vec![
            field("Program", format!("{:02X}:{:04X}", fx.pbr, self::pc(fx))),
            field("Cache base", hex(fx.cbr as u32, 16)),
            field("ROM data", format!("{:02X}:{:04X}", fx.rombr, fx.r[14])),
            field(
                "RAM data",
                format!("{:02X}  last {:04X}", 0x70 + fx.rambr, fx.ram_addr),
            ),
        ],
    );

    let screen = bitmap::Screen::of(fx);
    let (cols, grid_rows) = screen.tile_grid();
    let height = if fx.por.obj_mode() || fx.scmr.height() == 3 {
        "OBJ".to_string()
    } else {
        format!("{}px", grid_rows * 8)
    };
    let buffer_bytes = cols * grid_rows * (screen.bpp() << 3);
    let base = (fx.scbr as u32) << 10;
    let screen = section(
        "Screen",
        vec![
            field("Mode", format!("{}bpp  {height}", screen.bpp())),
            field(
                "Buffer",
                format!(
                    "70:{base:04X} .. {:04X}  ({} KB)",
                    base + buffer_bytes - 1,
                    buffer_bytes.div_ceil(1024)
                ),
            ),
            field(
                "Plot cursor",
                format!("({}, {})", fx.r[1] as u8, fx.r[2] as u8),
            ),
            Row::Flags {
                flags: vec![
                    FlagView::new("TRANS", fx.por.transparent()),
                    FlagView::new("DITH", fx.por.dither()),
                    FlagView::new("HI-NIB", fx.por.high_nibble()),
                    FlagView::new("FRZ", fx.por.freeze_high()),
                    FlagView::new("OBJ", fx.por.obj_mode()),
                ],
                note: Some((format!("COLR {:02X}", fx.colr.0).into(), Tone::Normal)),
            },
        ],
    );

    let grant = |gsu: bool| if gsu { "GSU" } else { "SNES" };
    let buffers = section(
        "Buses and buffers",
        vec![
            field(
                "Bus grants",
                format!(
                    "ROM {}  RAM {}",
                    grant(fx.scmr.rom_to_gsu()),
                    grant(fx.scmr.ram_to_gsu())
                ),
            ),
            field(
                "ROM buffer",
                match fx.rom_buffer.delay {
                    0 => format!("{:02X}", fx.rom_buffer.value),
                    delay => format!("{:02X}  fetch in {delay}", fx.rom_buffer.value),
                },
            ),
            field(
                "RAM queue",
                match fx.ram_buffer.delay {
                    0 => "idle".to_string(),
                    delay => format!(
                        "[{:04X}] = {:02X} in {delay}",
                        fx.ram_buffer.addr, fx.ram_buffer.value
                    ),
                },
            ),
            field("Prefixes", format!("SREG R{}  DREG R{}", fx.sreg, fx.dreg)),
        ],
    );

    let next_op = self::disasm_view(snes, Address::new(fx.pbr, fx.pipeline_addr).as_24bit(), 1)
        .into_iter()
        .next()
        .map(|line| line.text.to_string())
        .unwrap_or_default();
    let mut pipeline_rows = vec![
        field("Next op", format!("{next_op}  at {:04X}", fx.pipeline_addr)),
        field(
            "Loop",
            format!("R12 count {}  R13 target {:04X}", fx.r[12], fx.r[13]),
        ),
    ];
    if fx.pipeline_addr.wrapping_add(1) != fx.r[15] {
        pipeline_rows.push(Row::Field(Field {
            label: "Delay slot".into(),
            value: format!("then {:04X}", fx.r[15]).into(),
            accent: true,
        }));
    }
    let pipeline = section("Pipeline", pipeline_rows);

    let pixel = section(
        "Pixel caches",
        vec![
            Row::BitHeader { bits: 8 },
            Row::BitGrid {
                label: format!("1st ({}, {})", fx.pixel_primary.x, fx.pixel_primary.y).into(),
                mask: fx.pixel_primary.valid.into(),
            },
            Row::BitGrid {
                label: format!("2nd ({}, {})", fx.pixel_secondary.x, fx.pixel_secondary.y).into(),
                mask: fx.pixel_secondary.valid.into(),
            },
        ],
    );

    StructuredView {
        sections: vec![status, banks, screen, buffers, pipeline, pixel],
        meta: Some(format!("VCR {:02X}", crate::superfx::VERSION_CODE).into()),
    }
}

pub fn cache_view(snes: &Snes) -> TableView {
    let fx = &snes.superfx;

    let columns = vec![
        Column {
            label: "LINE".into(),
            width: ColWidth::Fixed(30.),
        },
        Column {
            label: "GSU".into(),
            width: ColWidth::Fixed(40.),
        },
        Column {
            label: "BYTES".into(),
            width: ColWidth::Flex,
        },
    ];

    let rows = (0..64u16)
        .map(|row| {
            let (line, half) = (row / 2, row % 2);
            let valid = fx.cache.valid & (1 << line) != 0;
            let start = line * 16 + half * 8;
            let bytes = format_bytes(8, |i| fx.cache.data[(start + i) as usize]);
            TableRow {
                active: valid,
                cells: vec![
                    TableCell::Text(if half == 0 {
                        format!("{line:02}").into()
                    } else {
                        "".into()
                    }),
                    TableCell::Text(format!("{:04X}", fx.cbr.wrapping_add(start)).into()),
                    TableCell::Text(bytes.into()),
                ],
            }
        })
        .collect();

    TableView {
        columns,
        rows,
        meta: Some(
            format!(
                "CBR {:04X}  {}/32 valid",
                fx.cbr,
                fx.cache.valid.count_ones()
            )
            .into(),
        ),
    }
}

pub fn bitmap_raster(snes: &Snes) -> RasterImage {
    let screen = bitmap::Screen::at_last_stop(&snes.superfx);
    let (cols, rows) = screen.tile_grid();
    let (width, height) = (cols * 8, rows * 8);
    let palette: Vec<_> = (0..16)
        .flat_map(|p| snes.ppu.palette(p))
        .map(crate::ppu::cgram::rgb)
        .collect();

    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let mut index = 0;
            for plane in screen.tile_row(x as u8, y as u8).planes() {
                let byte = snes.mmu.cartridge.sram_read(plane.addr as usize);
                index |= ((byte >> bitmap::pixel_bit(x as u8)) & 1) << plane.index;
            }

            let (r, g, b) = palette[index as usize];
            pixels.push([r, g, b, 255]);
        }
    }

    RasterImage {
        width,
        height,
        pixels: pixels.into(),
        details: vec![
            Field::new("Source", "Game Pak RAM"),
            Field::new("Depth", format!("{}bpp", screen.bpp())),
        ],
    }
}
