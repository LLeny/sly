use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSpec {
    pub sample_rate: u32,
    pub channels: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExecutionState {
    #[default]
    Paused,
    Running,
}

impl ExecutionState {
    pub const fn is_running(self) -> bool {
        matches!(self, Self::Running)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionInfo {
    pub system: Arc<str>,
    pub session: Arc<str>,
    pub segments: Vec<Arc<str>>,
}

#[derive(Clone, Copy, Debug)]
pub struct SoftwareFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: &'a [(u8, u8, u8)],
}

impl SoftwareFrame<'_> {
    pub fn is_valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.pixels.len() == self.width as usize * self.height as usize
    }
}

pub trait Core {
    fn reset(&mut self);
    fn step(&mut self);
    fn run_frame(&mut self);

    fn audio_spec(&self) -> Option<AudioSpec> {
        None
    }

    fn drain_audio(&mut self, _out: &mut Vec<i16>) {}

    fn session_info(&self) -> SessionInfo {
        SessionInfo::default()
    }

    fn software_frame(&self) -> Option<SoftwareFrame<'_>> {
        None
    }

    fn frame_interval(&self) -> Option<Duration> {
        None
    }
}
