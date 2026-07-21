use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::art_processing::color_saturation;
use crate::discovery::Library;
use crate::state::LauncherState;
use crate::ui::DetailBubble;

pub const CARDBOARD_SHADER_PATH: &str = "embedded://shaders/cardboard.wgsl";
const BACKGROUND_SHADER_PATH: &str = "embedded://shaders/background_contours.wgsl";
const FROSTED_BUBBLE_SHADER_PATH: &str = "embedded://shaders/frosted_bubble.wgsl";
const IRIS_BLACKOUT_SHADER_PATH: &str = "embedded://shaders/iris_blackout.wgsl";
pub const CONTOURS_SHADER_PATH: &str = "embedded://shaders/contours.wgsl";

pub const PATTERN_NEUTRAL_TINT: Vec3 = Vec3::new(0.44, 0.46, 0.49);
const CONTOUR_SCALE_PX: f32 = 430.0;

const FROSTED_BLUR_RADIUS_PX: f32 = 24.0;
const FROSTED_LIFT: f32 = 0.86;
const FROSTED_CORNER_RADIUS_PX: f32 = 26.0;

pub fn frosted_bubble_parameters(ui_scale: f32) -> Vec4 {
    Vec4::new(
        FROSTED_BLUR_RADIUS_PX * ui_scale,
        FROSTED_LIFT,
        FROSTED_CORNER_RADIUS_PX * ui_scale,
        ui_scale.max(1.0),
    )
}

pub type CardboardMaterial = ExtendedMaterial<StandardMaterial, CardboardExtension>;

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct BackgroundUiMaterial {
    #[uniform(0)]
    pub tint: Vec4,
    #[uniform(1)]
    pub parameters: Vec4,
}

impl UiMaterial for BackgroundUiMaterial {
    fn fragment_shader() -> ShaderRef {
        BACKGROUND_SHADER_PATH.into()
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct FrostedBubbleMaterial {
    #[uniform(0)]
    pub parameters: Vec4,
    #[uniform(1)]
    pub backdrop_tint: Vec4,
    #[uniform(2)]
    pub backdrop_parameters: Vec4,
    #[texture(3)]
    #[sampler(4)]
    pub shelf_view: Handle<Image>,
}

impl UiMaterial for FrostedBubbleMaterial {
    fn fragment_shader() -> ShaderRef {
        FROSTED_BUBBLE_SHADER_PATH.into()
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct IrisBlackoutMaterial {
    #[uniform(0)]
    pub parameters: Vec4,
}

impl UiMaterial for IrisBlackoutMaterial {
    fn fragment_shader() -> ShaderRef {
        IRIS_BLACKOUT_SHADER_PATH.into()
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct CardboardExtension {
    #[uniform(100)]
    pub parameters: Vec4,
}

impl MaterialExtension for CardboardExtension {
    fn fragment_shader() -> ShaderRef {
        CARDBOARD_SHADER_PATH.into()
    }
}

#[derive(Resource)]
pub struct BackgroundPattern {
    pub material: Handle<BackgroundUiMaterial>,
    pub tint: Vec3,
}

pub fn contour_parameters() -> Vec4 {
    Vec4::new(0.0, CONTOUR_SCALE_PX, 0.0, 0.0)
}

pub fn pattern_tint_uniform(tint: Vec3) -> Vec4 {
    Color::srgb(tint.x, tint.y, tint.z).to_linear().to_vec4()
}

pub fn pattern_tint_for_box(color: Vec3) -> Vec3 {
    let peak = color.max_element();
    if peak < 0.05 || color_saturation(color) < 0.04 {
        return PATTERN_NEUTRAL_TINT;
    }

    let target_peak = peak.clamp(0.38, 0.62);
    let adjusted = color / peak * target_peak;
    PATTERN_NEUTRAL_TINT.lerp(adjusted, 0.92)
}

pub fn animate_background_pattern(
    time: Res<Time>,
    launcher: Res<LauncherState>,
    library: Res<Library>,
    mut pattern: ResMut<BackgroundPattern>,
    mut materials: ResMut<Assets<BackgroundUiMaterial>>,
    mut frosted_materials: ResMut<Assets<FrostedBubbleMaterial>>,
    bubbles: Query<&MaterialNode<FrostedBubbleMaterial>, With<DetailBubble>>,
) {
    let target = library
        .art
        .get(launcher.selected)
        .and_then(|art| art.dominant_color)
        .map(pattern_tint_for_box)
        .unwrap_or(PATTERN_NEUTRAL_TINT);

    let dt = time.delta_secs().min(0.1);
    let previous = pattern.tint;
    pattern.tint.smooth_nudge(&target, 2.8, dt);

    if pattern.tint == previous {
        return;
    }
    let tint = pattern_tint_uniform(pattern.tint);

    if let Some(mut material) = materials.get_mut(&pattern.material) {
        material.tint = tint;
    }

    for handle in &bubbles {
        if let Some(mut material) = frosted_materials.get_mut(handle) {
            material.backdrop_tint = tint;
        }
    }
}
