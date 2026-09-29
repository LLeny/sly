use chapa::{FieldInfo, FieldKind};
use debugger_model::{Highlight, MmioFieldView, MmioGroup, MmioRegisterView, MmioView};

use crate::Gba;
use crate::mmio::mask;

pub(super) fn view(core: &Gba) -> MmioView {
    let bus = core.bus_view();
    let mut groups: Vec<MmioGroup> = Vec::new();

    for register in core.mmio.map.registers() {
        let info = register.info();
        let value = register.inspect(&bus);
        let row = MmioRegisterView {
            id: format!("gba.mmio.{:08X}", info.address).into(),
            name: info.name.into(),
            addr: format!("{:08X}", info.address).into(),
            value,
            bits: info.bits,
            highlight: Highlight::None,
            fields: fields(info.fields, value),
        };

        match groups
            .iter_mut()
            .find(|group| group.name.as_ref() == info.group)
        {
            Some(group) => group.registers.push(row),
            None => groups.push(MmioGroup {
                name: info.group.into(),
                registers: vec![row],
            }),
        }
    }

    MmioView { groups }
}

fn fields(metadata: &[FieldInfo], raw: u32) -> Vec<MmioFieldView> {
    let mut metadata: Vec<_> = metadata.iter().collect();
    metadata.sort_by_key(|field| field.offset);

    metadata
        .into_iter()
        .map(|field| {
            let value = (raw >> field.offset) & mask(field.width as u8);
            let numeric = || format!("0x{value:X}");

            let (display, children) = match field.kind {
                FieldKind::Bool => {
                    let state = if value == 0 { "off" } else { "on" };
                    (state.to_owned(), Vec::new())
                }
                FieldKind::Sint => {
                    let shift = 32 - field.width;
                    ((((value << shift) as i32) >> shift).to_string(), Vec::new())
                }
                FieldKind::Enum(info) => {
                    let name = info
                        .variants
                        .iter()
                        .find(|(number, _)| *number == u128::from(value))
                        .map_or_else(numeric, |(_, name)| (*name).to_owned());
                    (name, Vec::new())
                }
                FieldKind::Struct(nested) => (numeric(), fields(nested, value)),
                _ => (numeric(), Vec::new()),
            };

            let bit_label = if field.width == 1 {
                format!("[{}]", field.offset)
            } else {
                format!("[{}:{}]", field.offset, field.offset + field.width - 1)
            };

            MmioFieldView {
                name: field.name.into(),
                bit_label: bit_label.into(),
                value,
                display: display.into(),
                is_bool: matches!(field.kind, FieldKind::Bool),
                children,
            }
        })
        .collect()
}
