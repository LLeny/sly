use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::audio::AudioSource;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::shader::Shader;
use bevy::world_serialization::WorldAsset;

use crate::materials::{
    BackgroundPattern, BackgroundUiMaterial, CONTOURS_SHADER_PATH, PATTERN_NEUTRAL_TINT,
    contour_parameters, pattern_tint_uniform,
};

#[derive(Resource)]
pub struct Models {
    pub box_scene: Handle<WorldAsset>,
    pub jp_pal_cart_scene: Handle<WorldAsset>,
    pub us_cart_scene: Handle<WorldAsset>,
}

#[derive(Resource)]
pub struct Fonts {
    pub sans: Handle<Font>,
    pub sans_semibold: Handle<Font>,
    pub mono_medium: Handle<Font>,
}

#[derive(Resource)]
pub struct UiSounds {
    pub scroll: Handle<AudioSource>,
    pub game_start: Handle<AudioSource>,
}

#[derive(Resource)]
pub struct PromptIcons {
    pub arrow_left: Handle<Image>,
    pub arrow_right: Handle<Image>,
    pub key_f: Handle<Image>,
    pub key_tab: Handle<Image>,
    pub key_enter: Handle<Image>,
    pub key_escape: Handle<Image>,
    pub swipe_horizontal: Handle<Image>,
    pub swipe_vertical: Handle<Image>,
    pub tap: Handle<Image>,
}

#[derive(Resource)]
pub struct Icons {
    pub calendar: Handle<Image>,
    pub developer: Handle<Image>,
    pub publisher: Handle<Image>,
    pub genre: Handle<Image>,
    pub players: Handle<Image>,
    pub play: Handle<Image>,
    pub flip: Handle<Image>,
    pub view: Handle<Image>,
    pub save: Handle<Image>,
    pub load: Handle<Image>,
    pub library: Handle<Image>,
}

#[derive(Resource)]
pub struct GameTexture(pub Handle<Image>);

#[derive(Resource)]
pub struct ShaderLibrary {
    _contours: Handle<Shader>,
}

fn embed_assets(registry: &EmbeddedAssetRegistry) {
    let files: &[(&str, &'static [u8])] = &[
        (
            "models/snes-western-box.glb",
            include_bytes!("../../../vendor/models/snes-western-box.glb"),
        ),
        (
            "models/snes-jp-pal.glb",
            include_bytes!("../../../vendor/models/snes-jp-pal.glb"),
        ),
        (
            "models/snes-us.glb",
            include_bytes!("../../../vendor/models/snes-us.glb"),
        ),
        (
            "shaders/cardboard.wgsl",
            include_bytes!("../shaders/cardboard.wgsl"),
        ),
        (
            "shaders/background_contours.wgsl",
            include_bytes!("../shaders/background_contours.wgsl"),
        ),
        (
            "shaders/frosted_bubble.wgsl",
            include_bytes!("../shaders/frosted_bubble.wgsl"),
        ),
        (
            "shaders/iris_blackout.wgsl",
            include_bytes!("../shaders/iris_blackout.wgsl"),
        ),
        (
            "shaders/contours.wgsl",
            include_bytes!("../shaders/contours.wgsl"),
        ),
        (
            "sounds/scroll.ogg",
            include_bytes!("../../../vendor/sounds/click_003.ogg"),
        ),
        (
            "sounds/game-start.wav",
            include_bytes!("../../../vendor/sounds/game-start.wav"),
        ),
        (
            "prompts/keyboard_arrow_left.png",
            include_bytes!("../../../vendor/input-prompts/keyboard_arrow_left.png"),
        ),
        (
            "prompts/keyboard_arrow_right.png",
            include_bytes!("../../../vendor/input-prompts/keyboard_arrow_right.png"),
        ),
        (
            "prompts/keyboard_f.png",
            include_bytes!("../../../vendor/input-prompts/keyboard_f.png"),
        ),
        (
            "prompts/keyboard_tab.png",
            include_bytes!("../../../vendor/input-prompts/keyboard_tab_icon.png"),
        ),
        (
            "prompts/keyboard_enter.png",
            include_bytes!("../../../vendor/input-prompts/keyboard_numpad_enter.png"),
        ),
        (
            "prompts/keyboard_escape.png",
            include_bytes!("../../../vendor/input-prompts/keyboard_escape.png"),
        ),
        (
            "prompts/touch_swipe_horizontal.png",
            include_bytes!("../../../vendor/input-prompts/touch_swipe_horizontal.png"),
        ),
        (
            "prompts/touch_swipe_vertical.png",
            include_bytes!("../../../vendor/input-prompts/touch_swipe_vertical.png"),
        ),
        (
            "prompts/touch_tap.png",
            include_bytes!("../../../vendor/input-prompts/touch_tap.png"),
        ),
        (
            "icons/calendar.png",
            include_bytes!("../../../vendor/icons/lucide/calendar.png"),
        ),
        (
            "icons/user.png",
            include_bytes!("../../../vendor/icons/lucide/user.png"),
        ),
        (
            "icons/building-2.png",
            include_bytes!("../../../vendor/icons/lucide/building-2.png"),
        ),
        (
            "icons/tag.png",
            include_bytes!("../../../vendor/icons/lucide/tag.png"),
        ),
        (
            "icons/users.png",
            include_bytes!("../../../vendor/icons/lucide/users.png"),
        ),
        (
            "icons/play.png",
            include_bytes!("../../../vendor/icons/lucide/play.png"),
        ),
        (
            "icons/rotate-cw.png",
            include_bytes!("../../../vendor/icons/lucide/rotate-cw.png"),
        ),
        (
            "icons/eye.png",
            include_bytes!("../../../vendor/icons/lucide/eye.png"),
        ),
        (
            "icons/save.png",
            include_bytes!("../../../vendor/icons/lucide/save.png"),
        ),
        (
            "icons/history.png",
            include_bytes!("../../../vendor/icons/lucide/history.png"),
        ),
        (
            "icons/list.png",
            include_bytes!("../../../vendor/icons/lucide/list.png"),
        ),
        (
            "fonts/IBMPlexSans-Regular.ttf",
            include_bytes!("../../../vendor/fonts/ibm-plex/IBMPlexSans-Regular.ttf"),
        ),
        (
            "fonts/IBMPlexSans-SemiBold.ttf",
            include_bytes!("../../../vendor/fonts/ibm-plex/IBMPlexSans-SemiBold.ttf"),
        ),
        (
            "fonts/IBMPlexMono-Medium.ttf",
            include_bytes!("../../../vendor/fonts/ibm-plex/IBMPlexMono-Medium.ttf"),
        ),
    ];

    for &(path, bytes) in files {
        registry.insert_asset(PathBuf::new(), Path::new(path), bytes);
    }
}

pub fn setup_assets(
    mut commands: Commands,
    assets: Res<AssetServer>,
    embedded: Res<EmbeddedAssetRegistry>,
    mut images: ResMut<Assets<Image>>,
    mut background_materials: ResMut<Assets<BackgroundUiMaterial>>,
) {
    embed_assets(&embedded);

    commands.insert_resource(Models {
        box_scene: assets
            .load(GltfAssetLabel::Scene(0).from_asset("embedded://models/snes-western-box.glb")),
        jp_pal_cart_scene: assets
            .load(GltfAssetLabel::Scene(0).from_asset("embedded://models/snes-jp-pal.glb")),
        us_cart_scene: assets
            .load(GltfAssetLabel::Scene(0).from_asset("embedded://models/snes-us.glb")),
    });

    commands.insert_resource(Fonts {
        sans: assets.load("embedded://fonts/IBMPlexSans-Regular.ttf"),
        sans_semibold: assets.load("embedded://fonts/IBMPlexSans-SemiBold.ttf"),
        mono_medium: assets.load("embedded://fonts/IBMPlexMono-Medium.ttf"),
    });

    commands.insert_resource(UiSounds {
        scroll: assets.load("embedded://sounds/scroll.ogg"),
        game_start: assets.load("embedded://sounds/game-start.wav"),
    });

    commands.insert_resource(PromptIcons {
        arrow_left: assets.load("embedded://prompts/keyboard_arrow_left.png"),
        arrow_right: assets.load("embedded://prompts/keyboard_arrow_right.png"),
        key_f: assets.load("embedded://prompts/keyboard_f.png"),
        key_tab: assets.load("embedded://prompts/keyboard_tab.png"),
        key_enter: assets.load("embedded://prompts/keyboard_enter.png"),
        key_escape: assets.load("embedded://prompts/keyboard_escape.png"),
        swipe_horizontal: assets.load("embedded://prompts/touch_swipe_horizontal.png"),
        swipe_vertical: assets.load("embedded://prompts/touch_swipe_vertical.png"),
        tap: assets.load("embedded://prompts/touch_tap.png"),
    });

    commands.insert_resource(Icons {
        calendar: assets.load("embedded://icons/calendar.png"),
        developer: assets.load("embedded://icons/user.png"),
        publisher: assets.load("embedded://icons/building-2.png"),
        genre: assets.load("embedded://icons/tag.png"),
        players: assets.load("embedded://icons/users.png"),
        play: assets.load("embedded://icons/play.png"),
        flip: assets.load("embedded://icons/rotate-cw.png"),
        view: assets.load("embedded://icons/eye.png"),
        save: assets.load("embedded://icons/save.png"),
        load: assets.load("embedded://icons/history.png"),
        library: assets.load("embedded://icons/list.png"),
    });

    commands.insert_resource(ShaderLibrary {
        _contours: assets.load(CONTOURS_SHADER_PATH),
    });

    let mut image = Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    commands.insert_resource(GameTexture(images.add(image)));

    commands.insert_resource(BackgroundPattern {
        material: background_materials.add(BackgroundUiMaterial {
            tint: pattern_tint_uniform(PATTERN_NEUTRAL_TINT),
            parameters: contour_parameters(),
        }),
        tint: PATTERN_NEUTRAL_TINT,
    });
}
