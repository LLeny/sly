use std::time::Duration;

use bevy::prelude::*;

use core_model::Core;

use crate::assets::{GameTexture, Models};
use crate::cartridge::{DetailCart, swap_detail_cart};
use crate::discovery::Library;
use crate::launch::PLAYING_FADE_DURATION;
use crate::state::{LauncherState, ModeTimers};
use crate::ui::{GameFrame, GameUi, LauncherUi, show_game_screen};

pub const FRAME_WIDTH: u32 = 256;
pub const FRAME_HEIGHT: u32 = 224;

const MAX_CATCH_UP_FRAMES: u32 = 3;

#[derive(Resource)]
pub struct GameSession {
    pub core: Box<dyn Core + Send + Sync>,
    pub session: storage::Session<storage::FsStore>,
    pub audio_samples: Vec<i16>,
    pub frame_dirty: bool,
}

impl GameSession {
    pub fn save_state(&self) {
        if let Err(err) = self.session.save_state(&*self.core, storage::Slot::Quick) {
            warn!("failed to save state: {err}");
        }
    }

    pub fn load_state(&mut self) {
        match self
            .session
            .load_state(&mut *self.core, storage::Slot::Quick)
        {
            Ok(true) => self.frame_dirty = true,
            Ok(false) => {}
            Err(err) => warn!("failed to load state: {err}"),
        }
    }

    pub fn persist_save_ram(&self) {
        if let Err(err) = self.session.persist_save_ram(&*self.core) {
            warn!("failed to write save RAM: {err}");
        }
    }
}

impl Drop for GameSession {
    fn drop(&mut self) {
        self.persist_save_ram();
    }
}

const SAVE_RAM_FLUSH: Duration = Duration::from_secs(60);

pub fn flush_save_ram(session: Res<GameSession>, time: Res<Time>, mut due: Local<Duration>) {
    *due += time.delta();
    if *due < SAVE_RAM_FLUSH {
        return;
    }

    *due = Duration::ZERO;
    session.persist_save_ram();
}

pub struct AudioState(pub Option<audio::AudioPlayer>);

pub fn cap_core_backlog(mut fixed: ResMut<Time<Fixed>>) {
    let cap = fixed.timestep() * MAX_CATCH_UP_FRAMES;
    let overstep = fixed.overstep();
    if overstep > cap {
        fixed.discard_overstep(overstep - cap);
    }
}

pub fn step_core(mut session: ResMut<GameSession>, mut audio_state: NonSendMut<AudioState>) {
    let session = &mut *session;

    session.core.run_frame();
    session.frame_dirty = true;

    session.core.drain_audio(&mut session.audio_samples);
    if let Some(audio) = audio_state.0.as_mut() {
        audio.queue(&session.audio_samples);
    }
    session.audio_samples.clear();
}

pub fn present_game_frame(
    time: Res<Time>,
    mut timers: ResMut<ModeTimers>,
    session: Option<ResMut<GameSession>>,
    game_texture: Res<GameTexture>,
    mut images: ResMut<Assets<Image>>,
) {
    if timers.playing_elapsed < PLAYING_FADE_DURATION {
        timers.playing_elapsed += time.delta_secs();
    }

    let Some(mut session) = session else {
        return;
    };
    if !session.frame_dirty {
        return;
    }
    session.frame_dirty = false;

    let pixels = session
        .core
        .software_frame()
        .expect("core frame buffer disappeared")
        .pixels;
    let Some(mut image) = images.get_mut(&game_texture.0) else {
        return;
    };

    let data = image.data.get_or_insert_default();
    data.resize(pixels.len() * 4, 255);
    for (dst, &(r, g, b)) in data.as_chunks_mut::<4>().0.iter_mut().zip(pixels) {
        dst.copy_from_slice(&[r, g, b, 255]);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn exit_playing(
    mut commands: Commands,
    mut state: ResMut<LauncherState>,
    mut timers: ResMut<ModeTimers>,
    library: Res<Library>,
    models: Res<Models>,
    mut carts: Query<&mut DetailCart>,
    mut launcher_ui: Query<&mut Node, (With<LauncherUi>, Without<GameUi>)>,
    mut game_ui: Query<&mut Node, (With<GameUi>, Without<LauncherUi>)>,
    mut audio_state: NonSendMut<AudioState>,
) {
    commands.remove_resource::<GameSession>();
    audio_state.0 = None;

    timers.launch_elapsed = 0.0;
    timers.playing_elapsed = 0.0;
    state.box_flipped = false;
    state.status = "Returned to library".into();

    show_game_screen(false, &mut launcher_ui, &mut game_ui);
    swap_detail_cart(&mut commands, &mut carts, state.selected, &library, &models);
}

pub fn resize_game_frame(
    window: Single<&Window>,
    ui_scale: Res<UiScale>,
    mut frame: Single<&mut Node, With<GameFrame>>,
) {
    let scale = (window.width() / FRAME_WIDTH as f32)
        .min(window.height() / FRAME_HEIGHT as f32)
        .max(1.0)
        .floor();

    let width = px(FRAME_WIDTH as f32 * scale / ui_scale.0);
    if frame.width != width {
        frame.width = width;
        frame.height = px(FRAME_HEIGHT as f32 * scale / ui_scale.0);
    }
}
