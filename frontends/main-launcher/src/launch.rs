use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy::tasks::Task;
use bevy::tasks::futures::check_ready;
use core_model::Core;
use snes::Snes;

use crate::camera::DetailCamera;
use crate::cartridge::{CartMotion, DetailCart};
use crate::game::{AudioState, GameSession};
use crate::materials::IrisBlackoutMaterial;
use crate::scene_textures::CartridgeLabelSurface;
use crate::state::{LauncherMode, LauncherState, ModeTimers};
use crate::ui::{
    BUBBLE_MARGIN_PX, BUBBLE_VIEWPORT_FRACTION, BUBBLE_WIDTH_FRACTION, DetailFacts, DetailViewport,
    GameUi, LauncherUi, VIEWPORT_Z_IDLE, show_game_screen,
};

pub const LAUNCH_CLOSEUP_DURATION: f32 = 0.64;
const LAUNCH_IRIS_START: f32 = 0.76;
const LAUNCH_IRIS_DURATION: f32 = 0.68;
const LAUNCH_DURATION: f32 = 1.54;
pub const PLAYING_FADE_DURATION: f32 = 0.26;
pub const IRIS_DEFAULT_CENTER: Vec2 = Vec2::new(0.5, 0.36);
const LAUNCH_VIEWPORT_Z: i32 = 10;

#[derive(Resource, Default)]
pub struct PendingGame {
    pub core: Option<Snes>,
    pub error: Option<String>,
}

#[derive(Component, Default, Clone)]
pub struct Blackout;

#[derive(Component)]
pub struct GameLoadTask(pub Task<Result<Snes, String>>);

pub fn poll_game_load(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut GameLoadTask)>,
    mut pending: ResMut<PendingGame>,
) {
    for (entity, mut task) in &mut tasks {
        let Some(result) = check_ready(&mut task.0) else {
            continue;
        };
        commands.entity(entity).despawn();
        match result {
            Ok(core) => pending.core = Some(core),
            Err(err) => pending.error = Some(err),
        }
    }
}

pub fn advance_launch(
    time: Res<Time>,
    mut commands: Commands,
    mut state: ResMut<LauncherState>,
    mut timers: ResMut<ModeTimers>,
    mut pending: ResMut<PendingGame>,
    mut next_mode: ResMut<NextState<LauncherMode>>,
    mut fixed_time: ResMut<Time<Fixed>>,
    mut audio_state: NonSendMut<AudioState>,
) {
    timers.launch_elapsed += time.delta_secs();

    if let Some(error) = pending.error.take() {
        next_mode.set(LauncherMode::Browsing);
        state.status = error;
        return;
    }
    if timers.launch_elapsed < LAUNCH_DURATION {
        return;
    }

    let Some(core) = pending.core.take() else {
        state.status = "Still loading cartridge".into();
        return;
    };

    let frame_interval = core
        .frame_interval()
        .expect("SNES core reports a region-correct frame interval");
    fixed_time.set_timestep(frame_interval);
    audio_state.0 = core
        .audio_spec()
        .and_then(|spec| audio::AudioPlayer::new(spec).ok());
    commands.insert_resource(GameSession {
        core: Box::new(core),
        audio_samples: Vec::new(),
        frame_dirty: false,
    });

    next_mode.set(LauncherMode::Playing);
}

pub fn enter_playing(
    mut state: ResMut<LauncherState>,
    mut timers: ResMut<ModeTimers>,
    mut launcher_ui: Query<&mut Node, (With<LauncherUi>, Without<GameUi>)>,
    mut game_ui: Query<&mut Node, (With<GameUi>, Without<LauncherUi>)>,
) {
    timers.playing_elapsed = 0.0;
    state.status = "Press Escape to return to library".into();
    show_game_screen(true, &mut launcher_ui, &mut game_ui);
}

#[allow(clippy::type_complexity)]
pub fn update_launch_viewport(
    timers: Res<ModeTimers>,
    window: Single<&Window>,
    mut viewport: Single<
        (&mut Node, &mut GlobalZIndex),
        (With<DetailViewport>, Without<DetailFacts>),
    >,
    mut facts: Single<&mut Node, (With<DetailFacts>, Without<DetailViewport>)>,
) {
    let progress = launch_viewport_progress(timers.launch_elapsed);
    let window_width = window.width();
    let window_height = window.height();
    let bubble_width = window_width * BUBBLE_WIDTH_FRACTION;
    let bubble_height = (window_height - 2.0 * BUBBLE_MARGIN_PX).max(1.0);
    let bubble_left = window_width - BUBBLE_MARGIN_PX - bubble_width;
    let initial_height = bubble_height * BUBBLE_VIEWPORT_FRACTION;

    viewport.0.left = px(-bubble_left * progress);
    viewport.0.top = px(-BUBBLE_MARGIN_PX * progress);
    viewport.0.width = px(bubble_width + (window_width - bubble_width) * progress);
    viewport.0.height = px(initial_height + (window_height - initial_height) * progress);
    viewport.1.0 = LAUNCH_VIEWPORT_Z;
    facts.display = Display::None;
}

#[allow(clippy::type_complexity)]
pub fn restore_detail_viewport(
    mut viewport: Single<
        (&mut Node, &mut GlobalZIndex),
        (With<DetailViewport>, Without<DetailFacts>),
    >,
    mut facts: Single<&mut Node, (With<DetailFacts>, Without<DetailViewport>)>,
) {
    viewport.0.left = percent(0);
    viewport.0.top = percent(0);
    viewport.0.width = percent(100);
    viewport.0.height = percent(BUBBLE_VIEWPORT_FRACTION * 100.0);
    viewport.1.0 = VIEWPORT_Z_IDLE;
    facts.display = Display::Flex;
}

fn launch_viewport_progress(elapsed: f32) -> f32 {
    EaseFunction::SmoothStep.sample_clamped(elapsed / LAUNCH_CLOSEUP_DURATION)
}

fn launch_iris_progress(elapsed: f32) -> f32 {
    EaseFunction::SmoothStep.sample_clamped((elapsed - LAUNCH_IRIS_START) / LAUNCH_IRIS_DURATION)
}

#[allow(clippy::too_many_arguments)]
pub fn update_blackout(
    mode: Res<State<LauncherMode>>,
    state: Res<LauncherState>,
    timers: Res<ModeTimers>,
    blackout: Single<&MaterialNode<IrisBlackoutMaterial>, With<Blackout>>,
    mut materials: ResMut<Assets<IrisBlackoutMaterial>>,
    window: Single<&Window>,
    detail_camera: Single<(&Camera, &GlobalTransform), With<DetailCamera>>,
    viewport: Single<(&ComputedNode, &UiGlobalTransform), With<DetailViewport>>,
    label_surfaces: Query<(&CartridgeLabelSurface, &GlobalTransform, &Aabb)>,
    carts: Query<&DetailCart>,
) {
    let (iris, opacity) = match mode.get() {
        LauncherMode::Launching => (launch_iris_progress(timers.launch_elapsed), 1.0),
        LauncherMode::Playing => (
            1.0,
            (1.0 - timers.playing_elapsed / PLAYING_FADE_DURATION).clamp(0.0, 1.0),
        ),
        _ => (0.0, 1.0),
    };

    let mut center = materials
        .get(&**blackout)
        .map(|material| Vec2::new(material.parameters.y, material.parameters.z))
        .unwrap_or(IRIS_DEFAULT_CENTER);

    if *mode.get() == LauncherMode::Launching {
        let (camera, camera_transform) = *detail_camera;
        let label_surface = label_surfaces.iter().find(|(surface, _, _)| {
            carts.get(surface.cart_root).is_ok_and(|cart| {
                cart.index == state.selected && matches!(cart.motion, CartMotion::Launching)
            })
        });

        if let Some((_, label_transform, label_bounds)) = label_surface
            && let (Some(camera_size), Ok(label_position)) = (
                camera.logical_viewport_size(),
                camera.world_to_viewport(
                    camera_transform,
                    label_transform.transform_point(label_bounds.center.into()),
                ),
            )
        {
            let (viewport_node, viewport_transform) = *viewport;
            let inverse_scale = viewport_node.inverse_scale_factor();
            let viewport_size = viewport_node.size() * inverse_scale;
            let viewport_center = viewport_transform.translation * inverse_scale;
            let viewport_top_left = viewport_center - viewport_size * 0.5;
            let screen_position = viewport_top_left + label_position / camera_size * viewport_size;
            center = (screen_position / Vec2::new(window.width(), window.height()))
                .clamp(Vec2::splat(0.01), Vec2::splat(0.99));
        }
    }

    let parameters = Vec4::new(iris, center.x, center.y, opacity);
    if let Some(mut material) = materials.get_mut(&**blackout)
        && material.parameters != parameters
    {
        material.parameters = parameters;
    }
}
