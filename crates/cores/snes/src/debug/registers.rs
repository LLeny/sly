use super::*;

pub(super) fn mmio_component(addr: u16) -> (u8, &'static str) {
    match addr {
        0x2100..=0x213F => (0, "PPU"),
        0x2140..=0x2143 => (1, "APU"),
        0x420B..=0x420C | 0x4300..=0x43FF => (2, "DMA"),
        0x4200..=0x42FF => (3, "CPU"),
        0x2200..=0x230E => (4, "SA-1"),
        0x3000..=0x34FF => (5, "GSU"),
        _ => (9, "Other"),
    }
}

fn widen(addr: u16, bits: u8, mut peek: impl FnMut(u16) -> u8) -> u32 {
    let lo = peek(addr) as u32;
    if bits > 8 {
        lo | (peek(addr.wrapping_add(1)) as u32) << 8
    } else {
        lo
    }
}

pub(super) fn read_mmio(core: &Snes, addr: u16, bits: u8) -> u32 {
    self::widen(addr, bits, |addr| core.peek_mmio(addr))
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

pub(super) fn dma_view(core: &Snes) -> StructuredView {
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
                Field::new("B-bus", b_bus),
                Field::new("Pattern", format!("{} ({offsets})", pattern as u8)),
                Field::new("Step", step),
                Field::new(
                    "HDMA mode",
                    if ch.params.indirect() {
                        "indirect"
                    } else {
                        "direct"
                    },
                ),
                Field::new(
                    "HDMA table",
                    format!(
                        "${:?}",
                        Address::new(ch.a_bus_addr.table_bank(), ch.table_addr)
                    ),
                ),
                Field::new(
                    "HDMA lines",
                    format!(
                        "{}{}",
                        line.line_count(),
                        if line.repeat() { " (repeat)" } else { "" }
                    ),
                ),
                Field::new(
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

            let active = (mdmaen | hdmaen) & (1 << i) != 0;
            Row::Group {
                id: format!("dma-{i}").into(),
                title: format!("Channel {i}").into(),
                active,
                summary: format!(
                    "{source} -> {dest}  {}{}",
                    hex(ch.byte_count.amount() as u32, 16),
                    if hdmaen & (1 << i) != 0 { " HDMA" } else { "" }
                )
                .into(),
                rows: details.into_iter().map(Row::Field).collect(),
            }
        })
        .collect();

    StructuredView {
        sections: vec![Section {
            rows: channels,
            ..Default::default()
        }],
        meta: Some(format!("{} active", (mdmaen | hdmaen).count_ones()).into()),
    }
}

pub(super) fn mmio_view(core: &Snes) -> MmioView {
    let has_gsu = core.mmu.cartridge.coprocessor() == crate::cartridge_header::Coprocessor::SuperFx;
    let has_sa1 = core.mmu.cartridge.coprocessor() == crate::cartridge_header::Coprocessor::Sa1;

    let mut symbols: Vec<_> = crate::mmio::symbols()
        .map(|symbol| (self::mmio_component(symbol.addr), symbol))
        .filter(|((_, component), _)| match *component {
            "GSU" => has_gsu,
            "SA-1" => has_sa1,
            _ => true,
        })
        .collect();
    symbols.sort_unstable_by_key(|((order, _), symbol)| (*order, symbol.addr));

    let mut groups: Vec<MmioGroup> = Vec::new();
    for ((_, component), sym) in symbols {
        let value = if component == "SA-1" {
            self::widen(sym.addr, sym.bits, |addr| core.sa1.peek_register(addr))
        } else {
            self::read_mmio(core, sym.addr, sym.bits)
        };
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
