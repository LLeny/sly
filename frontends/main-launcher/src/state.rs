use bevy::prelude::*;

#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LauncherMode {
    #[default]
    Loading,
    Browsing,
    Launching,
    Playing,
}

#[derive(Resource, Default)]
pub struct LauncherState {
    pub selected: usize,
    pub box_flipped: bool,
    pub status: String,
}

#[derive(Resource, Default)]
pub struct ModeTimers {
    pub launch_elapsed: f32,
    pub playing_elapsed: f32,
}
