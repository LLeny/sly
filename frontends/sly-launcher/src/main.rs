mod art_processing;
mod artwork;
mod assets;
mod camera;
mod cartridge;
mod discovery;
mod game;
mod gamedb;
mod input;
mod launch;
mod library;
mod materials;
mod scene_textures;
mod shelf;
mod state;
mod ui;

use bevy::app::{RunFixedMainLoop, RunFixedMainLoopSystems};
use bevy::log::LogPlugin;
use bevy::prelude::*;

use camera::{
    CART_EXPOSURE_EV100, CART_LAYER, DetailCamera, SHELF_EXPOSURE_EV100, SHELF_LAYER,
    ViewCameraSpec, spawn_view_camera,
};
use game::AudioState;
use materials::{
    BackgroundUiMaterial, CardboardMaterial, FrostedBubbleMaterial, IrisBlackoutMaterial,
    PATTERN_NEUTRAL_TINT, contour_parameters, frosted_bubble_parameters, pattern_tint_uniform,
};
use state::{LauncherMode, LauncherState, ModeTimers};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(LogPlugin {
                    filter: "info".into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Sly Launcher".into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(MaterialPlugin::<CardboardMaterial>::default())
        .add_plugins(UiMaterialPlugin::<BackgroundUiMaterial>::default())
        .add_plugins(UiMaterialPlugin::<FrostedBubbleMaterial>::default())
        .add_plugins(UiMaterialPlugin::<IrisBlackoutMaterial>::default())
        .insert_resource(ClearColor(ui::BG_ROOT))
        .init_state::<LauncherMode>()
        .init_resource::<LauncherState>()
        .init_resource::<ModeTimers>()
        .init_resource::<discovery::Library>()
        .init_resource::<shelf::RevealBurst>()
        .init_resource::<launch::PendingGame>()
        .insert_non_send(AudioState(None))
        .add_systems(
            Startup,
            (
                assets::setup_assets,
                setup_scene,
                discovery::start_discovery,
            )
                .chain(),
        )
        .add_observer(scene_textures::on_scene_ready)
        .add_systems(OnEnter(LauncherMode::Playing), launch::enter_playing)
        .add_systems(OnExit(LauncherMode::Playing), game::exit_playing)
        .add_systems(
            OnExit(LauncherMode::Launching),
            launch::restore_detail_viewport,
        )
        .add_systems(
            RunFixedMainLoop,
            game::cap_core_backlog
                .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop)
                .run_if(in_state(LauncherMode::Playing)),
        )
        .add_systems(
            FixedUpdate,
            game::step_core.run_if(
                in_state(LauncherMode::Playing).and_then(resource_exists::<game::GameSession>),
            ),
        )
        .add_systems(
            Update,
            (
                ui::update_ui_scale,
                discovery::poll_discovery,
                discovery::poll_artwork,
                scene_textures::refresh_scene_materials,
                input::handle_keyboard,
                input::game_input.run_if(in_state(LauncherMode::Playing)),
                ui::apply_view_mode,
                shelf::animate_library,
                cartridge::animate_carts,
                materials::animate_background_pattern,
                ui::tint_pills,
                launch::poll_game_load,
                launch::advance_launch.run_if(in_state(LauncherMode::Launching)),
                launch::update_launch_viewport.run_if(in_state(LauncherMode::Launching)),
                game::present_game_frame.run_if(in_state(LauncherMode::Playing)),
                ui::update_ui,
                ui::update_alphabet_highlight,
                game::resize_game_frame,
                launch::update_blackout,
            )
                .chain(),
        )
        .run();
}

#[allow(clippy::too_many_arguments)]
fn setup_scene(
    mut commands: Commands,
    fonts: Res<assets::Fonts>,
    prompts: Res<assets::PromptIcons>,
    game_texture: Res<assets::GameTexture>,
    background_pattern: Res<materials::BackgroundPattern>,
    mut images: ResMut<Assets<Image>>,
    mut frosted_materials: ResMut<Assets<FrostedBubbleMaterial>>,
    mut iris_materials: ResMut<Assets<IrisBlackoutMaterial>>,
) {
    commands.spawn(Camera2d);

    let shelf_focus = Vec3::new(0.0, 0.015, 0.0);
    let (library_camera, library_view) = spawn_view_camera(
        &mut commands,
        &mut images,
        ViewCameraSpec {
            layer: SHELF_LAYER,
            transform: Transform::from_xyz(0.0, 0.025, 0.68).looking_at(shelf_focus, Vec3::Y),
            ev100: SHELF_EXPOSURE_EV100,
            clear: Color::linear_rgba(0.0, 0.0, 0.0, 0.0),
        },
    );
    let cart_focus = Vec3::new(0.0, 0.025, 0.0);
    let (detail_camera, _) = spawn_view_camera(
        &mut commands,
        &mut images,
        ViewCameraSpec {
            layer: CART_LAYER,
            transform: Transform::from_xyz(0.0, 0.035, 0.50).looking_at(cart_focus, Vec3::Y),
            ev100: CART_EXPOSURE_EV100,
            clear: Color::linear_rgba(0.936, 0.945, 0.958, 0.0),
        },
    );
    commands.entity(detail_camera).insert(DetailCamera);

    let frosted_bubble = frosted_materials.add(FrostedBubbleMaterial {
        parameters: frosted_bubble_parameters(1.0),
        backdrop_tint: pattern_tint_uniform(PATTERN_NEUTRAL_TINT),
        backdrop_parameters: contour_parameters(),
        shelf_view: library_view,
    });
    let iris_blackout = iris_materials.add(IrisBlackoutMaterial {
        parameters: Vec4::new(
            0.0,
            launch::IRIS_DEFAULT_CENTER.x,
            launch::IRIS_DEFAULT_CENTER.y,
            1.0,
        ),
    });

    camera::spawn_lights(&mut commands);
    shelf::spawn_row(&mut commands);

    commands.spawn_scene(ui::launcher_ui(
        &fonts,
        &prompts,
        &background_pattern,
        library_camera,
        detail_camera,
        frosted_bubble,
    ));
    commands.spawn_scene(ui::game_ui(&game_texture, &fonts, &prompts));
    commands.spawn_scene(ui::blackout_ui(iris_blackout));
}
