use super::*;

pub(super) fn caption(label: &str, snes: &Snes) -> String {
    format!(
        "{label} @ {:02X}:{:04X}",
        snes.cpu.program_bank, snes.cpu.pc
    )
}

pub(super) fn frame_interval(snes: &Snes) -> Duration {
    let clock_hz = snes.scheduler.master_clock_hz();
    let nanos = snes.scheduler.cycles_per_frame() as u128 * 1_000_000_000 / clock_hz as u128;
    Duration::from_nanos(nanos as u64)
}

pub(super) fn cartridge_view(cart: &Cartridge) -> CartridgeView {
    let speed = match cart.memory_map() {
        Ok(mm) => match mm.speed() {
            Speed::Fast => "Fast (3.58 MHz)",
            Speed::Slow => "Slow (2.68 MHz)",
        },
        Err(_) => "unknown",
    };

    CartridgeView {
        title: cart.title().into(),
        details: StructuredView {
            sections: vec![Section {
                rows: vec![
                    Field::new("Mapper", self::mapper_name(cart)).accented(),
                    Field::new("Speed", speed),
                    Field::new("Region", cart.region().to_string()),
                    Field::new("ROM size", core_model::format_byte_size(cart.rom.len())),
                    Field::new("SRAM size", core_model::format_byte_size(cart.sram.len())),
                    Field::new("Header", format!("${:06X}", cart.header_offset())),
                ]
                .into_iter()
                .map(Row::Field)
                .collect(),
                ..Default::default()
            }],
            meta: None,
        },
        cover: None,
    }
}

pub(super) fn mapper_name(cart: &Cartridge) -> &'static str {
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
