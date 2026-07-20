use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;

use crate::assets::Models;
use crate::camera::SHELF_LAYER;
use crate::library::BoxStyle;
use crate::scene_textures::{BoxTextures, SceneRenderLayer};
use crate::state::LauncherState;

const BOX_PITCH: f32 = 0.053;
const BOX_SCALE: f32 = 1.48;
const ROW_FOCUS_ANCHOR_X: f32 = -0.155;
const BOX_FOCUS_X: f32 = 0.018;
const BOX_FOCUS_Z: f32 = 0.135;
const BOX_WAVE_HEIGHT: f32 = 0.004;
const BOX_FOCUS_WAVE_HEIGHT: f32 = 0.007;
const BOX_SWAY_ANGLE: f32 = 0.028;
const BOX_FOCUS_SWAY_ANGLE: f32 = 0.014;
const ANIMATED_WINDOW: usize = 16;
const BOB_SPEED: f32 = 1.35;
const BOB_STAGGER: f32 = 0.58;
const APPROACH_RATE: f32 = 11.0;

#[derive(Component)]
pub struct RowRoot;

#[derive(Component)]
pub struct BoxIndex(pub usize);

#[derive(Component, Clone, Copy)]
pub struct ShelfBoxStyle(pub BoxStyle);

pub fn spawn_row(commands: &mut Commands) {
    commands.spawn((Transform::default(), Visibility::default(), RowRoot));
}

pub fn spawn_box(
    commands: &mut Commands,
    row: Entity,
    index: usize,
    style: BoxStyle,
    models: &Models,
) -> Entity {
    let entity = commands
        .spawn((
            WorldAssetRoot(models.box_scene.clone()),
            Transform::from_xyz(index as f32 * BOX_PITCH, 0.0, 0.0)
                .with_rotation(idle_box_rotation())
                .with_scale(box_base_scale(style) * BOX_SCALE),
            BoxIndex(index),
            ShelfBoxStyle(style),
            BoxTextures::default(),
            SceneRenderLayer(SHELF_LAYER),
            Visibility::Hidden,
        ))
        .id();

    commands.entity(row).add_child(entity);
    entity
}

pub fn animate_library(
    time: Res<Time>,
    state: Res<LauncherState>,
    mut row: Query<&mut Transform, (With<RowRoot>, Without<BoxIndex>)>,
    mut boxes: Query<(&BoxIndex, &ShelfBoxStyle, &mut Transform), Without<RowRoot>>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();

    if let Ok(mut transform) = row.single_mut() {
        let target = ROW_FOCUS_ANCHOR_X - state.selected as f32 * BOX_PITCH;
        transform
            .translation
            .x
            .smooth_nudge(&target, APPROACH_RATE, dt);
    }

    for (index, style, mut transform) in &mut boxes {
        if index.0.abs_diff(state.selected) > ANIMATED_WINDOW {
            continue;
        }

        let selected = index.0 == state.selected;
        let sway = box_sway(index.0, elapsed, selected);
        let target_rotation = if selected {
            let face = if state.box_flipped {
                Quat::from_rotation_y(PI) * focused_box_rotation(style.0)
            } else {
                focused_box_rotation(style.0)
            };
            Quat::from_rotation_y(sway)
                * face
                * Quat::from_rotation_z((elapsed * 0.65).cos() * 0.008)
        } else {
            Quat::from_rotation_y(sway) * idle_box_rotation()
        };
        let target_x = index.0 as f32 * BOX_PITCH + if selected { BOX_FOCUS_X } else { 0.0 };
        let target_y = box_wave(index.0, elapsed, selected);
        let target_z = if selected {
            BOX_FOCUS_Z + (elapsed * 1.05).cos() * 0.003
        } else {
            0.0
        };
        let target_scale = box_base_scale(style.0) * BOX_SCALE * if selected { 1.065 } else { 1.0 };

        transform.translation.smooth_nudge(
            &Vec3::new(target_x, target_y, target_z),
            APPROACH_RATE,
            dt,
        );
        transform
            .rotation
            .smooth_nudge(&target_rotation, APPROACH_RATE, dt);
        transform
            .scale
            .smooth_nudge(&target_scale, APPROACH_RATE, dt);
    }
}

fn box_bob_phase(index: usize, elapsed: f32) -> f32 {
    elapsed * BOB_SPEED - index as f32 * BOB_STAGGER
}

fn box_wave(index: usize, elapsed: f32, selected: bool) -> f32 {
    let amplitude = if selected {
        BOX_FOCUS_WAVE_HEIGHT
    } else {
        BOX_WAVE_HEIGHT
    };
    box_bob_phase(index, elapsed).sin() * amplitude
}

fn box_sway(index: usize, elapsed: f32, selected: bool) -> f32 {
    let amplitude = if selected {
        BOX_FOCUS_SWAY_ANGLE
    } else {
        BOX_SWAY_ANGLE
    };
    box_bob_phase(index, elapsed).cos() * amplitude
}

fn idle_box_rotation() -> Quat {
    Quat::from_rotation_y(FRAC_PI_2) * Quat::from_rotation_z(FRAC_PI_2)
}

fn focused_box_rotation(style: BoxStyle) -> Quat {
    let tilt = Quat::from_euler(EulerRot::XYZ, 0.018, 0.11, 0.026);
    match style {
        BoxStyle::Western => tilt,
        BoxStyle::Japanese => tilt * Quat::from_rotation_z(FRAC_PI_2),
    }
}

fn box_base_scale(style: BoxStyle) -> Vec3 {
    match style {
        BoxStyle::Western => Vec3::ONE,
        BoxStyle::Japanese => Vec3::new(190.0 / 179.0, 107.0 / 128.0, 1.0),
    }
}
