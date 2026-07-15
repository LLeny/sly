#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSpec {
    pub sample_rate: u32,
    pub channels: u8,
}

pub trait Core {
    fn reset(&mut self);
    fn step(&mut self);
    fn run_frame(&mut self);

    fn audio_spec(&self) -> Option<AudioSpec> {
        None
    }

    fn drain_audio(&mut self, _out: &mut Vec<i16>) {}
}
