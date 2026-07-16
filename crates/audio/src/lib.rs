use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Producer, Split};

pub use core_model::AudioSpec;

pub struct AudioPlayer {
    _stream: cpal::Stream,
    producer: ringbuf::HeapProd<i16>,
    channels: usize,
    ratio: f64,
    pos: f64,
    prev: [i16; 2],
    resampled: Vec<i16>,
}

impl AudioPlayer {
    pub fn new(spec: AudioSpec) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "no default output device".to_string())?;
        let config = device
            .default_output_config()
            .map_err(|e| format!("no default output config: {e}"))?;

        let device_rate = config.sample_rate();
        let device_channels = config.channels() as usize;
        let sample_format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();

        let capacity = (device_rate as usize / 5).max(2048) * 2;
        let (producer, mut consumer) = HeapRb::<i16>::new(capacity).split();

        let err_fn = |err| tracing::error!(?err, "audio output stream error");

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                stream_config,
                move |data: &mut [f32], _| {
                    fill(data, device_channels, &mut consumer, |s| s as f32 / 32768.0);
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                stream_config,
                move |data: &mut [i16], _| {
                    fill(data, device_channels, &mut consumer, |s| s);
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_output_stream(
                stream_config,
                move |data: &mut [u16], _| {
                    fill(data, device_channels, &mut consumer, |s| {
                        (s as i32 + 32768) as u16
                    });
                },
                err_fn,
                None,
            ),
            other => return Err(format!("unsupported sample format: {other:?}")),
        }
        .map_err(|e| format!("failed to build output stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("failed to start output stream: {e}"))?;

        Ok(Self {
            _stream: stream,
            producer,
            channels: spec.channels as usize,
            ratio: spec.sample_rate.max(1) as f64 / device_rate as f64,
            pos: 0.0,
            prev: [0; 2],
            resampled: Vec::new(),
        })
    }

    pub fn queue(&mut self, samples: &[i16]) {
        if self.channels == 0 {
            return;
        }

        self.resampled.clear();

        for frame in samples.chunks_exact(self.channels) {
            let cur = [frame[0], frame.get(1).copied().unwrap_or(frame[0])];

            let lerp =
                |a: i16, b: i16, t: f64| -> i16 { (a as f64 + (b as f64 - a as f64) * t) as i16 };

            while self.pos < 1.0 {
                self.resampled.push(lerp(self.prev[0], cur[0], self.pos));
                self.resampled.push(lerp(self.prev[1], cur[1], self.pos));
                self.pos += self.ratio;
            }

            self.pos -= 1.0;
            self.prev = cur;
        }

        self.producer.push_slice(&self.resampled);
    }
}

fn fill<T: Copy>(
    data: &mut [T],
    channels: usize,
    consumer: &mut ringbuf::HeapCons<i16>,
    convert: impl Fn(i16) -> T,
) {
    const BLOCK_FRAMES: usize = 64;

    let mut block = [0i16; BLOCK_FRAMES * 2];
    let silence = convert(0);

    for chunk in data.chunks_mut(channels * BLOCK_FRAMES) {
        let frames = chunk.len() / channels;
        let wanted = &mut block[..frames * 2];
        let got = consumer.pop_slice(wanted);
        wanted[got..].fill(0);

        for (frame, pair) in chunk.chunks_mut(channels).zip(wanted.as_chunks::<2>().0) {
            let (l, r) = (convert(pair[0]), convert(pair[1]));
            for (i, out) in frame.iter_mut().enumerate() {
                *out = match i {
                    0 => l,
                    1 => r,
                    _ => silence,
                };
            }
        }
    }
}
