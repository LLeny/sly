use debugger_model::{
    CpuDescriptor, CpuView, DisasmLine, DisassemblyRequest, FlagView, Row, Section, StructuredView,
};

use super::{disasm_view_on, field, fingerprint_65c816, flags_row, section};
use crate::Snes;
use crate::cpu::{CpuPeek, ExecutionState};
use crate::mmio::Address;
use crate::sa1::{Sa1, regs};

pub const CPU: &str = "cart.sa1";
pub const WORKSPACE: &str = "sa1";
pub const STATE_ID: &str = "sa1_state";

struct BusView<'a> {
    snes: &'a Snes,
}

impl CpuPeek for BusView<'_> {
    fn peek8(&self, addr: Address) -> u8 {
        self.snes.sa1.peek8(&self.snes.mmu.cartridge, addr)
    }
}

pub fn descriptor() -> CpuDescriptor {
    CpuDescriptor::new(CPU, "65C816", "sa1")
}

pub fn bus_peek(snes: &Snes, addr: Address) -> u8 {
    snes.sa1.peek8(&snes.mmu.cartridge, addr)
}

pub fn cpu_view(snes: &Snes) -> CpuView {
    let mut view = super::cpu_view(&snes.sa1.cpu);
    view.isa = "SA-1".into();
    view
}

pub fn disasm_view(snes: &Snes, request: &DisassemblyRequest) -> Vec<DisasmLine> {
    let decoded = super::decode_cpu(&snes.sa1.cpu, request.base);

    disasm_view_on(
        &decoded,
        &BusView { snes },
        request.base as u16,
        request.count,
    )
}

pub fn fingerprint(snes: &Snes, request: &DisassemblyRequest) -> u64 {
    let decoded = super::decode_cpu(&snes.sa1.cpu, request.base);
    fingerprint_65c816(
        &decoded,
        &BusView { snes },
        request.base as u16,
        request.count,
    )
}

fn register16(sa1: &Sa1, addr: u16) -> u16 {
    u16::from_le_bytes([
        sa1.peek_register(addr),
        sa1.peek_register(addr.wrapping_add(1)),
    ])
}

pub fn state_view(snes: &Snes) -> StructuredView {
    let sa1 = &snes.sa1;
    let ccnt = &sa1.ccnt;
    let execution = match (ccnt.reset(), ccnt.wait(), sa1.cpu.execution) {
        (true, _, _) => "held in reset",
        (_, true, _) => "held by CCNT",
        (_, _, ExecutionState::Running) => "running",
        (_, _, ExecutionState::Waiting) => "waiting for interrupt",
        (_, _, ExecutionState::Stopped) => "stopped",
    };
    let status = Section {
        leading: true,
        ..section(
            "Status",
            vec![
                field(
                    "Program",
                    format!("{:02X}:{:04X}", sa1.cpu.program_bank, sa1.cpu.pc),
                ),
                flags_row(
                    execution,
                    vec![
                        FlagView::new("RESET", ccnt.reset()),
                        FlagView::new("WAIT", ccnt.wait()),
                        FlagView::new("EMU", sa1.cpu.emulation),
                    ],
                ),
            ],
        )
    };

    let scnt = regs::SnesCpuControl::from_raw(sa1.peek_register(regs::SnesCpuControl::ADDRESS));
    let communication = section(
        "Communication",
        vec![
            field(
                "SNES to SA-1",
                format!("message ${:X}", ccnt.message_to_sa1()),
            ),
            field(
                "SA-1 to SNES",
                format!("message ${:X}", scnt.message_to_snes()),
            ),
        ],
    );

    let sa1_irqs = |nmi, dma, timer, snes| {
        vec![
            FlagView::new("NMI", nmi),
            FlagView::new("DMA", dma),
            FlagView::new("TIMER", timer),
            FlagView::new("SNES", snes),
        ]
    };
    let snes_irqs = |ccdma, from_sa1| {
        vec![
            FlagView::new("CCDMA", ccdma),
            FlagView::new("SA-1", from_sa1),
        ]
    };
    let cie = sa1.interrupt_enable();
    let cfr = regs::Sa1Flags::from_raw(sa1.peek_register(regs::Sa1Flags::ADDRESS));
    let sfr = sa1.sfr();
    let interrupts = section(
        "Interrupts",
        vec![
            flags_row(
                "SA-1 enables",
                sa1_irqs(
                    cie.nmi_from_snes(),
                    cie.dma_irq(),
                    cie.timer_irq(),
                    cie.irq_from_snes(),
                ),
            ),
            flags_row(
                "SA-1 pending",
                sa1_irqs(
                    cfr.nmi_from_snes(),
                    cfr.dma_irq(),
                    cfr.timer_irq(),
                    cfr.irq_from_snes(),
                ),
            ),
            flags_row(
                "SNES enables",
                snes_irqs(
                    sa1.sie.character_conversion_dma_irq(),
                    sa1.sie.irq_from_sa1(),
                ),
            ),
            flags_row(
                "SNES pending",
                snes_irqs(sfr.character_conversion_dma_irq(), sfr.irq_from_sa1()),
            ),
        ],
    );

    let vector = |label, addr| field(label, format!("${:04X}", register16(sa1, addr)));
    let vectors = section(
        "Vectors",
        vec![
            vector("SA-1 reset", regs::VECTOR_START),
            vector("SA-1 NMI", regs::VECTOR_START + 2),
            vector("SA-1 IRQ", regs::VECTOR_START + 4),
            vector("SNES NMI", regs::SNES_VECTOR_START),
            vector("SNES IRQ", regs::SNES_VECTOR_START + 2),
        ],
    );

    let control = &sa1.memory;
    let rom_bank = |window| {
        field(
            match window {
                0 => "ROM C",
                1 => "ROM D",
                2 => "ROM E",
                _ => "ROM F",
            },
            format!(
                "block {} • {}",
                control.rom_block(window),
                if control.lo_rom_enabled(window) {
                    "LoROM"
                } else {
                    "HiROM"
                }
            ),
        )
    };
    let block = |block: usize| format!("block {block} • ${:05X}", block << 13);
    let memory = section(
        "Memory control",
        vec![
            rom_bank(0),
            rom_bank(1),
            rom_bank(2),
            rom_bank(3),
            field("SNES BW-RAM", block(control.bwram_snes_block())),
            field(
                "SA-1 BW-RAM",
                if control.bwram_sa1_bitmap() {
                    format!("bitmap block {}", control.bwram_sa1_bitmap_block())
                } else {
                    block(control.bwram_sa1_block())
                },
            ),
            field(
                "SNES write override",
                enabled(control.bwram_snes_write_override()),
            ),
            field(
                "SA-1 write override",
                enabled(control.bwram_sa1_write_override()),
            ),
            field(
                "Protected through",
                format!("${:05X}", control.bwram_protected_bytes() - 1),
            ),
            Row::BitHeader { bits: 8 },
            Row::BitGrid {
                label: "SNES I-RAM".into(),
                mask: sa1.siwp.raw().into(),
            },
            Row::BitGrid {
                label: "SA-1 I-RAM".into(),
                mask: sa1.ciwp.raw().into(),
            },
        ],
    );

    let math_control =
        regs::ArithmeticControl::from_raw(sa1.peek_register(regs::ArithmeticControl::ADDRESS));
    let math_mode = match math_control.mode() {
        0 => "multiply",
        1 => "divide",
        _ => "multiply-sum",
    };
    let (operand_a, operand_b) = sa1.arithmetic_operands();
    let math = section(
        "Arithmetic",
        vec![
            field(
                "Mode",
                format!("{math_mode}  MCNT ${:02X}", math_control.raw()),
            ),
            field(
                "Operands",
                format!("A ${operand_a:04X}  B ${operand_b:04X}"),
            ),
            field("Result", format!("${:010X}", sa1.arithmetic_result())),
            field(
                "Overflow",
                format!("${:02X}", sa1.peek_register(regs::ARITHMETIC_OVERFLOW)),
            ),
        ],
    );

    let cdma = &sa1.cdma;
    let depth = match cdma.color_depth() {
        0 => "8 bpp",
        1 => "4 bpp",
        2 => "2 bpp",
        _ => "reserved",
    };
    let width = match cdma.virtual_vram_width() {
        value @ 0..=5 => (1_u8 << value).to_string(),
        _ => "reserved".to_string(),
    };
    let conversion = section(
        "Character conversion",
        vec![
            field("Format", format!("{depth}  width {width} chars")),
            flags_row(
                &format!("CDMA ${:02X}", cdma.raw()),
                vec![FlagView::new("TERMINATE", cdma.terminate())],
            ),
        ],
    );

    StructuredView {
        sections: vec![
            status,
            communication,
            interrupts,
            vectors,
            memory,
            math,
            conversion,
        ],
        meta: None,
    }
}

fn enabled(on: bool) -> String {
    if on { "enabled" } else { "disabled" }.to_owned()
}
