use debugger_model::{FlagView, Row, Section, StructuredView, hex};

use super::field;
use crate::Gba;
use crate::dma::regs::StartTiming;

pub(super) fn view(core: &Gba) -> StructuredView {
    let channels = (0..4)
        .map(|channel| {
            let control = core.dma.control[channel];
            let source = hex(core.dma.src_addr[channel].address(), 32);
            let destination = hex(core.dma.dest_addr[channel].address(), 32);
            let raw_count = core.dma.word_count[channel].raw();
            let count = core.dma.units(channel);
            let size = control.transfer_size().to_string();
            let timing = match (control.start_timing(), channel) {
                (StartTiming::Special, 0) => "Prohibited".to_string(),
                (StartTiming::Special, 1 | 2) => "Sound FIFO".to_string(),
                (StartTiming::Special, _) => "TODO ???".to_string(),
                (timing, _) => timing.to_string(),
            };
            let summary = format!("{source} -> {destination}  {count} x {size}  {timing}");

            let mut rows = vec![
                field("Source", source),
                field("Destination", destination),
                field("Source step", control.src_addr_ctrl().to_string()),
                field("Destination step", control.dest_addr_ctrl().to_string()),
                field("Transfer size", size),
                field("Start timing", timing),
                field(
                    "Unit count",
                    format!("{count} ({})", hex(raw_count.into(), 16)),
                ),
                field("Control", hex(control.raw().into(), 16)),
            ];
            let mut flags = vec![
                FlagView::new("Enabled", control.enable()),
                FlagView::new("Repeat", control.repeat()),
                FlagView::new("IRQ", control.irq_on_completion()),
            ];
            if channel == 3 {
                flags.push(FlagView::new("Game Pak DRQ", control.gamepak_drq()));
            }
            rows.push(Row::Flags { flags, note: None });

            Row::Group {
                id: format!("dma-{channel}").into(),
                title: format!("Channel {channel}").into(),
                summary: summary.into(),
                active: control.enable(),
                rows,
            }
        })
        .collect();

    let enabled = core
        .dma
        .control
        .iter()
        .filter(|control| control.enable())
        .count();
    StructuredView {
        sections: vec![Section {
            rows: channels,
            ..Default::default()
        }],
        meta: Some(format!("{enabled} enabled").into()),
    }
}
