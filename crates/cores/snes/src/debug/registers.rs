use super::*;

pub(super) fn mmio_component(addr: u16) -> (u8, &'static str) {
    match addr {
        0x2100..=0x213F => (0, "PPU"),
        0x2140..=0x2143 => (1, "APU"),
        0x420B..=0x420C | 0x4300..=0x43FF => (2, "DMA"),
        0x4200..=0x42FF => (3, "CPU"),
        0x3000..=0x34FF => (4, "GSU"),
        _ => (9, "Other"),
    }
}

pub(super) fn read_mmio(core: &Snes, addr: u16, bits: u8) -> u32 {
    let lo = core.peek_mmio(addr) as u32;
    if bits > 8 {
        lo | (core.peek_mmio(addr.wrapping_add(1)) as u32) << 8
    } else {
        lo
    }
}

pub(super) fn mmio_field_views(
    fields: &[crate::mmio::FieldInfo],
    parent: u32,
) -> Vec<MmioFieldView> {
    let mut ordered: Vec<_> = fields.iter().collect();
    ordered.sort_by_key(|f| f.offset);
    ordered
        .into_iter()
        .map(|f| self::mmio_field_view(f, parent))
        .collect()
}

pub(super) fn mmio_field_view(field: &crate::mmio::FieldInfo, parent: u32) -> MmioFieldView {
    use crate::mmio::FieldKind;

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

    let fmt_hex = || hex(value, field.width as u8);

    let (display, is_bool, children) = match field.kind {
        FieldKind::Bool => (
            if value != 0 { "on" } else { "off" }.to_string(),
            true,
            Vec::new(),
        ),
        FieldKind::Uint => (fmt_hex(), false, Vec::new()),
        FieldKind::Sint => {
            let sign_shift = 32 - field.width;
            let signed = ((value << sign_shift) as i32) >> sign_shift;
            (signed.to_string(), false, Vec::new())
        }
        FieldKind::Enum(info) => {
            let display = info
                .variants
                .iter()
                .find(|(disc, _)| *disc == value as u128)
                .map(|(_, name)| name.to_string())
                .unwrap_or_else(fmt_hex);
            (display, false, Vec::new())
        }
        FieldKind::Struct(fields) => (fmt_hex(), false, self::mmio_field_views(fields, value)),
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

pub(super) fn dma_view(core: &Snes) -> DmaView {
    use crate::mmio::dma::transfer_pattern_offsets;
    use crate::mmio::regs::{AddressAdjustMode, Direction};

    let mdmaen = core.dma.mdmaen.raw();
    let hdmaen = core.dma.hdmaen.raw();

    let channels = core
        .dma
        .channels
        .iter()
        .enumerate()
        .map(|(i, ch)| {
            let b_port = crate::mmio::dma::B_BUS_BASE | ch.b_bus_addr as u16;
            let a_side = format!("${:?}", Address::from(ch.a_bus_addr.raw()));
            let b_side = hex(b_port as u32, 16);
            let b_bus = match crate::mmio::symbol(b_port) {
                Some(name) => format!("{b_side} {name}"),
                None => b_side.clone(),
            };
            let (source, dest) = match ch.params.direction() {
                Direction::AToB => (a_side, b_side),
                Direction::BToA => (b_side, a_side),
            };

            let pattern = ch.params.transfer_pattern();
            let offsets = transfer_pattern_offsets(pattern)
                .iter()
                .map(|offset| format!("+{offset}"))
                .collect::<Vec<_>>()
                .join(" ");
            let step = match ch.params.address_adjust_mode() {
                AddressAdjustMode::Increment => "increment",
                AddressAdjustMode::Decrement => "decrement",
                AddressAdjustMode::Fixed => "fixed",
            };

            let line = ch.line_counter;
            let details = vec![
                DmaChannelDetail::new("B-bus", b_bus),
                DmaChannelDetail::new("Pattern", format!("{} ({offsets})", pattern as u8)),
                DmaChannelDetail::new("Step", step),
                DmaChannelDetail::new(
                    "HDMA mode",
                    if ch.params.indirect() {
                        "indirect"
                    } else {
                        "direct"
                    },
                ),
                DmaChannelDetail::new(
                    "HDMA table",
                    format!(
                        "${:?}",
                        Address::new(ch.a_bus_addr.table_bank(), ch.table_addr)
                    ),
                ),
                DmaChannelDetail::new(
                    "HDMA lines",
                    format!(
                        "{}{}",
                        line.line_count(),
                        if line.repeat() { " (repeat)" } else { "" }
                    ),
                ),
                DmaChannelDetail::new(
                    "Indirect address",
                    format!(
                        "${:?}",
                        Address::new(
                            ch.byte_count.indirect_bank(),
                            ch.byte_count.indirect_offset()
                        )
                    ),
                ),
            ];

            DmaChannelView {
                id: format!("dma-{i}").into(),
                name: format!("{i}").into(),
                dma_enabled: mdmaen & (1 << i) != 0,
                hdma_enabled: hdmaen & (1 << i) != 0,
                source: source.into(),
                dest: dest.into(),
                count: hex(ch.byte_count.amount() as u32, 16).into(),
                details,
            }
        })
        .collect();

    DmaView { channels }
}

pub(super) fn mmio_view(core: &Snes) -> MmioView {
    let has_gsu = core.mmu.cartridge.coprocessor() == crate::cartridge::Coprocessor::SuperFx;

    let mut symbols: Vec<_> = crate::mmio::symbols()
        .filter(|s| has_gsu || self::mmio_component(s.addr).1 != "GSU")
        .collect();
    symbols.sort_by_key(|s| (self::mmio_component(s.addr).0, s.addr));

    let mut groups: Vec<MmioGroup> = Vec::new();
    for sym in symbols {
        let (_, component) = self::mmio_component(sym.addr);
        let value = self::read_mmio(core, sym.addr, sym.bits);
        let reg = self::mmio_register("mmio", 4, sym, value);

        match groups.last_mut() {
            Some(group) if &*group.name == component => group.registers.push(reg),
            _ => groups.push(MmioGroup {
                name: component.into(),
                registers: vec![reg],
            }),
        }
    }

    let spc = MmioGroup {
        name: "SPC".into(),
        registers: crate::apu::symbols()
            .map(|sym| self::mmio_register("spc", 2, sym, core.apu.peek(sym.addr) as u32))
            .collect(),
    };
    let after_apu = groups
        .iter()
        .position(|g| &*g.name == "APU")
        .map_or(groups.len(), |ix| ix + 1);
    groups.insert(after_apu, spc);

    MmioView { groups }
}

pub(super) fn mmio_register(
    id_prefix: &str,
    addr_width: usize,
    sym: &crate::mmio::MmioSymbol,
    value: u32,
) -> MmioRegisterView {
    MmioRegisterView {
        id: format!("{id_prefix}-{:0width$X}", sym.addr, width = addr_width).into(),
        name: sym.name.into(),
        addr: hex(sym.addr as u32, (addr_width * 4) as u8).into(),
        value,
        bits: sym.bits,
        highlight: Highlight::None,
        fields: self::mmio_field_views(sym.fields, value),
    }
}
