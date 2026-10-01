use std::io::Cursor;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use audio::{AudioPlayer, AudioSpec};

const RATE: u32 = 44_100;
const BUFFER_AHEAD: Duration = Duration::from_millis(40);
const LAUNCH_PEAK: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Navigate,
    Adjust,
    Select,
    Back,
    Flip,
    View,
    Launch,
}

#[derive(Default)]
struct CueGate {
    last: Option<Instant>,
}

impl CueGate {
    fn accept(&mut self, cue: Cue, now: Instant) -> bool {
        let interval = Duration::from_millis(if matches!(cue, Cue::Navigate | Cue::Adjust) {
            100
        } else {
            50
        });

        if cue != Cue::Launch
            && self
                .last
                .is_some_and(|last| now.saturating_duration_since(last) < interval)
        {
            return false;
        }

        self.last = Some(now);
        true
    }
}

struct Clips {
    click: Vec<i16>,
    launch: Vec<i16>,
}

impl Clips {
    fn load() -> Result<Self> {
        let mut ogg = lewton::inside_ogg::OggStreamReader::new(Cursor::new(include_bytes!(
            "../../../vendor/sounds/click_003.ogg"
        )))
        .context("Cannot decode bundled selection cue")?;

        let click_rate = ogg.ident_hdr.audio_sample_rate;
        let click_channels = usize::from(ogg.ident_hdr.audio_channels);
        let mut click = Vec::new();

        while let Some(packet) = ogg.read_dec_packet_itl()? {
            click.extend(packet);
        }

        let mut wav = hound::WavReader::new(Cursor::new(include_bytes!(
            "../../../vendor/sounds/game-start.wav"
        )))
        .context("Cannot decode bundled launch cue")?;

        let spec = wav.spec();

        anyhow::ensure!(
            spec.sample_format == hound::SampleFormat::Int && spec.bits_per_sample == 16,
            "Unsupported bundled launch-cue encoding"
        );

        let launch = wav
            .samples::<i16>()
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut launch = stereo_at_rate(&launch, usize::from(spec.channels), spec.sample_rate);

        // The launch chord peaks near -20 dBFS; normalize it to -6 dBFS.
        let peak = launch
            .iter()
            .map(|sample| sample.unsigned_abs())
            .max()
            .unwrap_or(0);

        if peak > 0 {
            let gain = f32::from(i16::MAX) * LAUNCH_PEAK / f32::from(peak);

            for sample in &mut launch {
                *sample = (f32::from(*sample) * gain).round() as i16;
            }
        }

        Ok(Self {
            click: stereo_at_rate(&click, click_channels, click_rate),
            launch,
        })
    }

    fn samples(&self, cue: Cue) -> &[i16] {
        if cue == Cue::Launch {
            &self.launch
        } else {
            &self.click
        }
    }
}

fn stereo_at_rate(samples: &[i16], channels: usize, rate: u32) -> Vec<i16> {
    if channels == 0 || rate == 0 {
        return Vec::new();
    }

    let source_frames = samples.len() / channels;

    if source_frames == 0 {
        return Vec::new();
    }

    let frames = (source_frames as u64 * u64::from(RATE) / u64::from(rate)) as usize;
    let mut result = Vec::with_capacity(frames * 2);

    for frame in 0..frames {
        let position = frame as f64 * f64::from(rate) / f64::from(RATE);
        let previous = position.floor() as usize;
        let next = (previous + 1).min(source_frames - 1);
        let fraction = position.fract();

        for channel in [0, usize::from(channels > 1)] {
            let a = f64::from(samples[previous * channels + channel]);
            let b = f64::from(samples[next * channels + channel]);
            result.push((a + (b - a) * fraction).round() as i16);
        }
    }

    result
}

#[derive(Default)]
pub struct UiSounds {
    clips: Option<Clips>,
    output: Option<AudioPlayer>,
    active: Option<(Cue, usize)>,
    scratch: Vec<i16>,
    gate: CueGate,
    failed: bool,
}

impl UiSounds {
    pub fn play(&mut self, cue: Cue, volume: f32, now: Instant) {
        if volume <= 0.0 {
            self.stop();
            return;
        }

        if self
            .active
            .is_some_and(|(previous, _)| previous == Cue::Launch)
        {
            self.stop();
        }

        if self.failed || !self.gate.accept(cue, now) {
            return;
        }

        if self.clips.is_none() {
            match Clips::load() {
                Ok(clips) => self.clips = Some(clips),
                Err(error) => {
                    tracing::warn!(%error, "UI sounds unavailable");
                    self.failed = true;
                    return;
                }
            }
        }

        if self.output.is_none() {
            match AudioPlayer::new(AudioSpec {
                sample_rate: RATE,
                channels: 2,
            }) {
                Ok(output) => self.output = Some(output),
                Err(error) => {
                    tracing::warn!(%error, "UI sounds unavailable");
                    self.failed = true;
                    return;
                }
            }
        }

        self.active = Some((cue, 0));
        tracing::debug!(?cue, "UI sound");
        self.update(volume);
    }

    pub fn update(&mut self, volume: f32) {
        if volume <= 0.0 {
            self.stop();
            return;
        }

        let (Some(output), Some(clips), Some((cue, cursor))) =
            (&mut self.output, &self.clips, &mut self.active)
        else {
            return;
        };

        let samples = clips.samples(*cue);
        // Queue a little at a time; the shared ring buffer would truncate a long cue.
        let needed = BUFFER_AHEAD.saturating_sub(output.buffered());
        let wanted = ((needed.as_secs_f64() * f64::from(RATE)).ceil() as usize * 2)
            .min(samples.len().saturating_sub(*cursor));

        if wanted > 0 {
            let gain = volume.clamp(0.0, 1.0);
            self.scratch.clear();

            self.scratch.extend(
                samples[*cursor..*cursor + wanted]
                    .iter()
                    .map(|&sample| (f32::from(sample) * gain).round() as i16),
            );

            output.queue(&self.scratch);
            *cursor += wanted;
        }

        if *cursor >= samples.len() && output.buffered().is_zero() {
            self.active = None;
        }
    }

    pub fn stop(&mut self) {
        self.output = None;
        self.active = None;
    }
}
