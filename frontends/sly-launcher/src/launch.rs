use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy::tasks::futures::check_ready;
use bevy::tasks::{IoTaskPool, Task};
use core_model::Core;
use snes::Snes;

use crate::assets::UiSounds;
use crate::camera::DetailCamera;
use crate::cartridge::{CartMotion, DetailCart};
use crate::config::Config;
use crate::discovery::Library;
use crate::game::{AudioState, GameSession};
use crate::library;
use crate::materials::IrisBlackoutMaterial;
use crate::scene_textures::CartridgeLabelSurface;
use crate::state::{LauncherMode, LauncherState, ModeTimers, Paused, ViewMode};
use crate::touch::TouchControls;
use crate::ui::{
    DetailFacts, DetailViewport, GameUi, LauncherUi, SafeArea, VIEWPORT_Z_IDLE,
    detail_camera_transform, display, show_game_screen, style_viewport_idle, ui_sound,
    viewport_idle_rect,
};

pub const LAUNCH_CLOSEUP_DURATION: f32 = 0.64;
const LAUNCH_IRIS_START: f32 = 0.76;
const LAUNCH_IRIS_DURATION: f32 = 0.68;
const LAUNCH_DURATION: f32 = 1.54;
pub const PLAYING_FADE_DURATION: f32 = 0.26;
const RETURN_IRIS_DURATION: f32 = 0.58;
pub const IRIS_DEFAULT_CENTER: Vec2 = Vec2::new(0.5, 0.36);
const LAUNCH_VIEWPORT_Z: i32 = 10;

#[derive(Resource, Default)]
pub struct PendingGame {
    pub core: Option<(Snes, storage::GameId)>,
    pub error: Option<String>,
}

#[derive(Component, Default, Clone)]
pub struct Blackout;

#[derive(Component)]
pub struct GameLoadTask(pub Task<Result<(Snes, storage::GameId), String>>);

pub fn begin_launch(
    commands: &mut Commands,
    state: &mut LauncherState,
    timers: &mut ModeTimers,
    next_mode: &mut NextState<LauncherMode>,
    library: &Library,
    carts: &mut Query<&mut DetailCart>,
    ui_sounds: &UiSounds,
) {
    let Some(entry) = library.entries.get(state.selected) else {
        return;
    };

    let path = entry.path.clone();
    let game = storage::GameId::new(core_model::System::Snes, entry.sha1.clone());
    let task = IoTaskPool::get()
        .spawn(async move { library::load_cartridge(&path).map(|cart| (Snes::new(cart), game)) });
    commands.spawn(GameLoadTask(task));
    commands.spawn(ui_sound(ui_sounds.game_start.clone()));

    next_mode.set(LauncherMode::Launching);
    timers.launch_elapsed = 0.0;
    timers.return_active = false;
    state.status = "Booting selected cartridge".into();

    for mut cart in carts {
        if cart.index == state.selected {
            cart.motion = CartMotion::Launching;
        }
    }
}

pub fn return_to_library(
    session: Option<&GameSession>,
    next_mode: &mut NextState<LauncherMode>,
    timers: &mut ModeTimers,
) {
    if let Some(session) = session {
        session.persist_save_ram();
    }

    next_mode.set(LauncherMode::Browsing);

    timers.return_elapsed = 0.0;
    timers.return_active = true;
}

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

#[allow(clippy::too_many_arguments)]
pub fn advance_launch(
    time: Res<Time>,
    config: Res<Config>,
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

    let Some((mut core, game)) = pending.core.take() else {
        state.status = "Still loading cartridge".into();
        return;
    };

    let session = storage::Session::new(config.save_store(), game);
    if let Err(err) = session.restore_save_ram(&mut core) {
        warn!("failed to restore save RAM: {err}");
    }

    let frame_interval = core
        .frame_interval()
        .expect("SNES core reports a region-correct frame interval");
    fixed_time.set_timestep(frame_interval);
    audio_state.open(&core);
    commands.insert_resource(GameSession {
        core: Box::new(core),
        session,
        audio_samples: Vec::new(),
        frame_dirty: false,
    });

    next_mode.set(LauncherMode::Playing);
}

pub fn enter_playing(
    mut state: ResMut<LauncherState>,
    mut timers: ResMut<ModeTimers>,
    mut paused: ResMut<Paused>,
    mut launcher_ui: Query<&mut Node, (With<LauncherUi>, Without<GameUi>)>,
    mut game_ui: Query<&mut Node, (With<GameUi>, Without<LauncherUi>)>,
) {
    timers.playing_elapsed = 0.0;
    paused.0 = false;
    state.status = "Press Escape to return to library".into();
    show_game_screen(true, &mut launcher_ui, &mut game_ui);
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn update_launch_viewport(
    state: Res<LauncherState>,
    controls: Res<TouchControls>,
    timers: Res<ModeTimers>,
    window: Single<&Window>,
    ui_scale: Res<UiScale>,
    safe_area: Res<SafeArea>,
    mut viewport: Single<
        (&mut Node, &mut GlobalZIndex),
        (With<DetailViewport>, Without<DetailFacts>),
    >,
    mut facts: Single<&mut Node, (With<DetailFacts>, Without<DetailViewport>)>,
    mut camera: Single<&mut Transform, With<DetailCamera>>,
) {
    let progress = launch_viewport_progress(timers.launch_elapsed);
    let window_size = Vec2::new(window.width(), window.height()) / ui_scale.0;

    let insets = safe_area.0;
    let inset_start = insets.xy();
    let safe_size = window_size - inset_start - insets.zw();
    let (origin, idle_size) = viewport_idle_rect(state.view_mode, *controls, safe_size);
    let origin = origin + inset_start;

    viewport.0.left = px(-origin.x * progress);
    viewport.0.top = px(-origin.y * progress);
    viewport.0.width = px(idle_size.x + (window_size.x - idle_size.x) * progress);
    viewport.0.height = px(idle_size.y + (window_size.y - idle_size.y) * progress);
    viewport.1.0 = LAUNCH_VIEWPORT_Z;
    facts.display = Display::None;

    let default_camera = detail_camera_transform(ViewMode::Row);
    camera.translation = camera
        .translation
        .lerp(default_camera.translation, progress);
    camera.rotation = camera.rotation.slerp(default_camera.rotation, progress);
}

#[allow(clippy::type_complexity)]
pub fn restore_detail_viewport(
    state: Res<LauncherState>,
    controls: Res<TouchControls>,
    mut viewport: Single<
        (&mut Node, &mut GlobalZIndex),
        (With<DetailViewport>, Without<DetailFacts>),
    >,
    mut facts: Single<&mut Node, (With<DetailFacts>, Without<DetailViewport>)>,
    mut camera: Single<&mut Transform, With<DetailCamera>>,
) {
    style_viewport_idle(state.view_mode, *controls, &mut viewport.0);
    viewport.1.0 = VIEWPORT_Z_IDLE;
    facts.display = Display::Flex;
    **camera = detail_camera_transform(state.view_mode);
}

fn launch_viewport_progress(elapsed: f32) -> f32 {
    EaseFunction::SmoothStep.sample_clamped(elapsed / LAUNCH_CLOSEUP_DURATION)
}

fn launch_iris_progress(elapsed: f32) -> f32 {
    EaseFunction::SmoothStep.sample_clamped((elapsed - LAUNCH_IRIS_START) / LAUNCH_IRIS_DURATION)
}

fn return_iris_progress(elapsed: f32) -> f32 {
    EaseFunction::SmoothStep.sample_clamped(elapsed / RETURN_IRIS_DURATION)
}

#[allow(clippy::too_many_arguments)]
pub fn update_blackout(
    time: Res<Time>,
    mode: Res<State<LauncherMode>>,
    state: Res<LauncherState>,
    mut timers: ResMut<ModeTimers>,
    mut blackout: Single<(&MaterialNode<IrisBlackoutMaterial>, &mut Node), With<Blackout>>,
    mut materials: ResMut<Assets<IrisBlackoutMaterial>>,
    window: Single<&Window>,
    ui_scale: Res<UiScale>,
    detail_camera: Single<(&Camera, &GlobalTransform), With<DetailCamera>>,
    viewport: Single<(&ComputedNode, &UiGlobalTransform), With<DetailViewport>>,
    label_surfaces: Query<(&CartridgeLabelSurface, &GlobalTransform, &Aabb)>,
    carts: Query<&DetailCart>,
) {
    let returning = *mode.get() == LauncherMode::Browsing && timers.return_active;
    let (iris, opacity) = match mode.get() {
        LauncherMode::Launching => (launch_iris_progress(timers.launch_elapsed), 1.0),
        LauncherMode::Playing => (
            1.0,
            (1.0 - timers.playing_elapsed / PLAYING_FADE_DURATION).clamp(0.0, 1.0),
        ),
        LauncherMode::Browsing if returning => {
            timers.return_elapsed += time.delta_secs();
            if timers.return_elapsed >= RETURN_IRIS_DURATION {
                timers.return_active = false;
            }
            (1.0 - return_iris_progress(timers.return_elapsed), 1.0)
        }
        _ => (0.0, 1.0),
    };

    let (material_node, blackout_node) = &mut *blackout;
    let mut center = materials
        .get(&**material_node)
        .map(|material| Vec2::new(material.parameters.y, material.parameters.z))
        .unwrap_or(IRIS_DEFAULT_CENTER);

    let window_size = Vec2::new(window.width(), window.height()) / ui_scale.0;
    let (viewport_node, viewport_transform) = *viewport;
    let inverse_scale = viewport_node.inverse_scale_factor();
    let viewport_size = viewport_node.size() * inverse_scale;
    let viewport_center = viewport_transform.translation * inverse_scale;

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
            let viewport_top_left = viewport_center - viewport_size * 0.5;
            let screen_position = viewport_top_left + label_position / camera_size * viewport_size;
            center = (screen_position / window_size).clamp(Vec2::splat(0.01), Vec2::splat(0.99));
        }
    } else if returning {
        center = (viewport_center / window_size).clamp(Vec2::splat(0.01), Vec2::splat(0.99));
    }

    let parameters = Vec4::new(iris, center.x, center.y, opacity);
    if let Some(mut material) = materials.get_mut(&**material_node)
        && material.parameters != parameters
    {
        material.parameters = parameters;
    }

    let wanted_display = display(iris > 0.0 && opacity > 0.0);
    if blackout_node.display != wanted_display {
        blackout_node.display = wanted_display;
    }
}
