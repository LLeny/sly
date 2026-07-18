use std::sync::Arc;

use debugger_model::{
    AudioChannel, AudioScopesView, Cell, ColWidth, Column, Field, FlagView, Row, Section,
    StructuredView, TableRow, TableView, Tone, hex,
};

use crate::apu::dsp::registers::{
    R_ECHO_DELAY, R_ECHO_ENABLE, R_ECHO_FEEDBACK, R_ECHO_START_ADDRESS, R_ECHO_VOLUME_LEFT,
    R_ECHO_VOLUME_RIGHT, R_FIR_FIRST, R_FLAGS, R_KEY_OFF, R_KEY_ON, R_MASTER_VOLUME_LEFT,
    R_MASTER_VOLUME_RIGHT, R_NOISE_ENABLE, R_PITCH_MODULATION, R_SOURCE_DIRECTORY, R_VOICE_END,
    V_ADSR_CONTROL, V_SOURCE_NUMBER, V_VOLUME_LEFT, V_VOLUME_RIGHT, VOICE_COUNT,
    VOICE_REGISTER_STRIDE,
};
use crate::apu::dsp::{Dsp, VoiceDebug};

pub const VOICES_ID: &str = "dsp_voices";
pub const REGISTERS_ID: &str = "dsp_registers";
pub const INSPECTOR_ID: &str = "audio_inspector";
pub const SCOPES_ID: &str = "channel_scopes";

const SAMPLE_MAX: i32 = 32_768;

struct Voice {
    index: u8,
    active: bool,
    srcn: u8,
    pitch_hz: u32,
    effective_pitch_hz: u32,
    vol_l: i8,
    vol_r: i8,
    env_mode: &'static str,
    noise: bool,
    echo: bool,
    pmon: bool,
    vd: VoiceDebug,
}

fn voice(dsp: &Dsp, i: usize) -> Voice {
    let base = i * VOICE_REGISTER_STRIDE;
    let bit = 1u8 << i;
    let pitch = dsp.voice_pitch(i);
    let adsr0 = dsp.reg(base + V_ADSR_CONTROL);
    let vd = dsp.voice_debug(i);
    let hz = |pitch: i32| (32_000i64 * pitch.max(0) as i64 / 4096).clamp(0, u32::MAX as i64) as u32;

    Voice {
        index: i as u8,
        active: vd.env > 0,
        srcn: dsp.reg(base + V_SOURCE_NUMBER),
        pitch_hz: hz(pitch),
        effective_pitch_hz: hz(vd.effective_pitch),
        vol_l: dsp.reg(base + V_VOLUME_LEFT) as i8,
        vol_r: dsp.reg(base + V_VOLUME_RIGHT) as i8,
        env_mode: if adsr0 & 0x80 == 0 {
            "gain"
        } else {
            vd.env_mode
        },
        noise: dsp.reg(R_NOISE_ENABLE) & bit != 0,
        echo: dsp.reg(R_ECHO_ENABLE) & bit != 0,
        pmon: dsp.reg(R_PITCH_MODULATION) & bit != 0,
        vd,
    }
}

fn fmt_hz(hz: u32) -> String {
    if hz >= 10_000 {
        format!("{:.1}k", hz as f32 / 1000.0)
    } else {
        format!("{hz}")
    }
}

fn field(label: &str, value: String) -> Row {
    Row::Field(Field {
        label: label.into(),
        value: value.into(),
        accent: false,
    })
}

fn section(title: &str, rows: Vec<Row>) -> Section {
    Section {
        title: Some(title.into()),
        rows,
        ..Section::default()
    }
}

fn page_addr(page: u8) -> String {
    hex((page as u32) << 8, 16)
}

pub fn voices_view(dsp: &Dsp) -> TableView {
    let voices: Vec<Voice> = (0..VOICE_COUNT).map(|i| voice(dsp, i)).collect();
    let active = voices.iter().filter(|v| v.active).count();

    let columns = vec![
        Column {
            label: "CH".into(),
            width: ColWidth::Fixed(18.),
        },
        Column {
            label: "SRC".into(),
            width: ColWidth::Fixed(24.),
        },
        Column {
            label: "PITCH".into(),
            width: ColWidth::Fixed(44.),
        },
        Column {
            label: "VOL".into(),
            width: ColWidth::Fixed(46.),
        },
        Column {
            label: "ENV".into(),
            width: ColWidth::Flex,
        },
        Column {
            label: "FLAGS".into(),
            width: ColWidth::Fixed(36.),
        },
    ];

    let rows = voices
        .iter()
        .map(|v| TableRow {
            active: v.active,
            cells: vec![
                Cell::Text(format!("V{}", v.index).into()),
                Cell::Text(format!("{:02X}", v.srcn).into()),
                Cell::Text(fmt_hz(v.pitch_hz).into()),
                Cell::Text(format!("{}/{}", v.vol_l, v.vol_r).into()),
                Cell::Meter {
                    value: v.vd.envx as i32,
                    max: 127,
                    label: Some(v.env_mode.into()),
                },
                Cell::FlagChips(vec![
                    FlagView::new("N", v.noise),
                    FlagView::new("E", v.echo),
                    FlagView::new("P", v.pmon),
                ]),
            ],
        })
        .collect();

    TableView {
        columns,
        rows,
        meta: Some(format!("{active}/{VOICE_COUNT} active").into()),
    }
}

pub fn registers_view(dsp: &Dsp) -> StructuredView {
    let flg = dsp.reg(R_FLAGS);
    let esa = dsp.reg(R_ECHO_START_ADDRESS);
    let edl = dsp.reg(R_ECHO_DELAY);
    let fir: Vec<Arc<str>> = (0..8)
        .map(|t| {
            format!(
                "{:+}",
                dsp.reg(R_FIR_FIRST + t * VOICE_REGISTER_STRIDE) as i8
            )
            .into()
        })
        .collect();

    let status = Section {
        leading: true,
        ..section(
            "Status",
            vec![Row::Flags {
                flags: vec![
                    FlagView::new("RST", flg & 0x80 != 0),
                    FlagView::new("MUTE", flg & 0x40 != 0),
                    FlagView::new("ECHO", flg & 0x20 == 0),
                ],
                note: Some((format!("NOISE {:02}", flg & 0x1F).into(), Tone::Normal)),
            }],
        )
    };

    let mix = section(
        "Mix",
        vec![
            field(
                "Master L/R",
                format!(
                    "{:+}/{:+}",
                    dsp.reg(R_MASTER_VOLUME_LEFT) as i8,
                    dsp.reg(R_MASTER_VOLUME_RIGHT) as i8
                ),
            ),
            field(
                "Echo vol L/R",
                format!(
                    "{:+}/{:+}",
                    dsp.reg(R_ECHO_VOLUME_LEFT) as i8,
                    dsp.reg(R_ECHO_VOLUME_RIGHT) as i8
                ),
            ),
            field(
                "Echo feedback",
                format!("{:+}", dsp.reg(R_ECHO_FEEDBACK) as i8),
            ),
        ],
    );

    let echo = section(
        "Echo",
        vec![
            field("Buffer", page_addr(esa)),
            field("Delay", format!("{} ms", (edl as u32 & 0x0F) * 16)),
            field("Sample table", page_addr(dsp.reg(R_SOURCE_DIRECTORY))),
        ],
    );

    let fir_taps = section("FIR taps", vec![Row::Tags { tags: fir }]);

    let per_voice = section(
        "Voices",
        vec![
            Row::BitHeader { bits: 8 },
            bit_grid("KON", dsp.reg(R_KEY_ON)),
            bit_grid("KOFF", dsp.reg(R_KEY_OFF)),
            bit_grid("ENDX", dsp.reg(R_VOICE_END)),
            bit_grid("Noise", dsp.reg(R_NOISE_ENABLE)),
            bit_grid("Echo", dsp.reg(R_ECHO_ENABLE)),
            bit_grid("Pitch mod", dsp.reg(R_PITCH_MODULATION)),
        ],
    );

    StructuredView {
        sections: vec![status, mix, echo, fir_taps, per_voice],
        meta: None,
    }
}

fn bit_grid(label: &str, mask: u8) -> Row {
    Row::BitGrid {
        label: label.into(),
        bits: 8,
        on_mask: mask,
    }
}

pub fn inspector_view(dsp: &Dsp, selection: Option<usize>) -> StructuredView {
    let Some(v) = selection
        .filter(|&s| s < VOICE_COUNT)
        .map(|s| voice(dsp, s))
    else {
        return StructuredView::default();
    };

    let header = v.vd.brr_header;
    let range = header >> 4;
    let filter = (header >> 2) & 0x03;
    let end = header & 0x01 != 0;
    let looping = header & 0x02 != 0;
    let fir_sum: i16 = (0..8)
        .map(|t| dsp.reg(R_FIR_FIRST + t * VOICE_REGISTER_STRIDE) as i8 as i16)
        .sum();
    let esa = dsp.reg(R_ECHO_START_ADDRESS);
    let echo_delay_ms = (dsp.reg(R_ECHO_DELAY) as u32 & 0x0F) * 16;

    let (key_text, key_tone) = if v.vd.key_on_delay == 0 {
        ("ready".to_string(), Tone::Muted)
    } else {
        (format!("KON +{}", v.vd.key_on_delay), Tone::Warning)
    };

    let identity = Section {
        leading: true,
        rows: vec![
            Row::Flags {
                flags: vec![
                    FlagView::new("NOISE", v.noise),
                    FlagView::new("ECHO", v.echo),
                    FlagView::new("PMON", v.pmon),
                ],
                note: Some((key_text.into(), key_tone)),
            },
            field(
                "Echo path",
                format!("{}  {echo_delay_ms} ms  FIR {fir_sum:+}", page_addr(esa)),
            ),
        ],
        ..Section::default()
    };

    let signal = Section {
        group: 1,
        basis: Some(0.42),
        ..section(
            "Signal",
            vec![
                meter("Source", v.vd.source_sample, v.active),
                meter("Envelope", v.vd.output_sample, v.active),
                meter("Mix L", v.vd.mixed_sample[0], v.active),
                meter("Mix R", v.vd.mixed_sample[1], v.active),
            ],
        )
    };

    let state = Section {
        group: 1,
        ..section(
            "Voice state",
            vec![
                field(
                    "Envelope",
                    format!(
                        "{} {:03X}/7FF  H {:03X}",
                        v.env_mode, v.vd.env, v.vd.hidden_env
                    ),
                ),
                field(
                    "Pitch",
                    format!("{} → {} Hz", v.pitch_hz, v.effective_pitch_hz),
                ),
                field(
                    "BRR cursor",
                    format!(
                        "${:04X}+{}  R{range}/F{filter}  E{} L{}",
                        v.vd.brr_addr, v.vd.brr_offset, end as u8, looping as u8
                    ),
                ),
                field(
                    "Gaussian",
                    format!("phase {:04X}", v.vd.interpolation_position & 0x3FFF),
                ),
            ],
        )
    };

    StructuredView {
        sections: vec![identity, signal, state],
        meta: Some(format!("V{} / SRC {:02X}", v.index, v.srcn).into()),
    }
}

fn meter(label: &str, value: i32, active: bool) -> Row {
    Row::Meter {
        label: label.into(),
        value,
        max: SAMPLE_MAX,
        accent: active,
    }
}

pub fn scopes_view(dsp: &Dsp) -> AudioScopesView {
    let channels = (0..VOICE_COUNT)
        .map(|i| {
            let active = dsp.voice_debug(i).env > 0;
            AudioChannel {
                label: format!("V{i}").into(),
                active,
                waveform: if active {
                    dsp.voice_waveform(i)
                } else {
                    [].into()
                },
            }
        })
        .collect();
    AudioScopesView { channels }
}
