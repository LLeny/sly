use bevy::prelude::*;

#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LauncherMode {
    #[default]
    Loading,
    Browsing,
    Launching,
    Playing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    Row,
    Column,
    Ribbon,
}

impl ViewMode {
    pub fn next(self) -> Self {
        match self {
            ViewMode::Row => ViewMode::Column,
            ViewMode::Column => ViewMode::Ribbon,
            ViewMode::Ribbon => ViewMode::Row,
        }
    }
}

#[derive(Resource, Default)]
pub struct LauncherState {
    pub selected: usize,
    pub box_flipped: bool,
    pub view_mode: ViewMode,
    pub status: String,
}

#[derive(Resource, Default)]
pub struct ModeTimers {
    pub launch_elapsed: f32,
    pub playing_elapsed: f32,
}
