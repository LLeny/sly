use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;

use crate::assets::Models;
use crate::camera::SHELF_LAYER;
use crate::library::BoxStyle;
use crate::scene_textures::{BoxTextures, SceneRenderLayer};
use crate::state::{LauncherState, ViewMode};

const BOX_PITCH: f32 = 0.053;
const BOX_SCALE: f32 = 1.48;
const ROW_FOCUS_ANCHOR_X: f32 = -0.155;
const BOX_FOCUS_X: f32 = 0.018;
const BOX_FOCUS_Z: f32 = 0.135;
const COLUMN_PITCH: f32 = 0.050;
const COLUMN_X: f32 = -0.16;
const COLUMN_TOP_Y: f32 = 0.26;
const COLUMN_Z: f32 = 0.02;
const COLUMN_FOCUS: Vec3 = Vec3::new(-0.12, -0.05, 0.175);
const COLUMN_SPINE_TILT: f32 = 0.12;
const JP_SPINE_ALIGN: f32 = 0.0155;
const RIBBON_ROW_Y: f32 = 0.073;
const RIBBON_ROW_SCALE: f32 = 0.95;
const BOX_WAVE_HEIGHT: f32 = 0.004;
const BOX_FOCUS_WAVE_HEIGHT: f32 = 0.007;
const BOX_SWAY_ANGLE: f32 = 0.028;
const BOX_FOCUS_SWAY_ANGLE: f32 = 0.014;
const ANIMATED_WINDOW: usize = 16;
const BOB_SPEED: f32 = 1.35;
const BOB_STAGGER: f32 = 0.58;
const APPROACH_RATE: f32 = 11.0;
const ENTER_RATE: f32 = 2.4;
const ENTER_SCALE_FLOOR: f32 = 0.62;
const ENTER_DEPTH: f32 = 0.045;
const STAGGER_PER: f32 = 0.03;
const STAGGER_CAP: usize = 24;
const REVEAL_BURST_WINDOW: f32 = 0.3;

#[derive(Component)]
pub struct RowRoot;

#[derive(Component)]
pub struct BoxIndex(pub usize);

#[derive(Component, Clone, Copy)]
pub struct ShelfBoxStyle(pub BoxStyle);

#[derive(Component)]
pub struct BoxEntrance {
    pub progress: f32,
    pub delay: f32,
}

#[derive(Resource)]
pub struct RevealBurst {
    last_reveal: f32,
    count: usize,
}

impl Default for RevealBurst {
    fn default() -> Self {
        Self {
            last_reveal: f32::NEG_INFINITY,
            count: 0,
        }
    }
}

impl RevealBurst {
    pub fn next_delay(&mut self, now: f32) -> f32 {
        if now - self.last_reveal <= REVEAL_BURST_WINDOW {
            self.count += 1;
        } else {
            self.count = 0;
        }
        self.last_reveal = now;
        self.count.min(STAGGER_CAP) as f32 * STAGGER_PER
    }
}

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
            Transform::from_xyz(0.0, 0.0, -ENTER_DEPTH)
                .with_rotation(idle_box_rotation())
                .with_scale(box_base_scale(style) * BOX_SCALE * ENTER_SCALE_FLOOR),
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
    mut boxes: Query<
        (
            &BoxIndex,
            &ShelfBoxStyle,
            &mut Transform,
            Option<&mut BoxEntrance>,
        ),
        Without<RowRoot>,
    >,
    mut revealed: Local<Vec<usize>>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();
    let mode = state.view_mode;

    revealed.clear();
    revealed.extend(
        boxes
            .iter()
            .filter_map(|(index, _, _, entrance)| entrance.is_some().then_some(index.0)),
    );
    revealed.sort_unstable();
    let slot_of = |index: usize| revealed.partition_point(|&r| r < index);

    let selected_slot = slot_of(state.selected);
    let row_target = row_focus_target(mode, selected_slot);

    if let Ok(mut transform) = row.single_mut() {
        transform
            .translation
            .smooth_nudge(&row_target, APPROACH_RATE, dt);
        transform
            .scale
            .smooth_nudge(&Vec3::splat(row_display_scale(mode)), APPROACH_RATE, dt);
    }

    let slot_axis = match mode {
        ViewMode::Row | ViewMode::Ribbon => Vec3::new(BOX_PITCH, 0.0, 0.0),
        ViewMode::Column => Vec3::new(0.0, -COLUMN_PITCH, 0.0),
    };
    let idle_western = mode_idle_rotation(mode, BoxStyle::Western);
    let idle_japanese = mode_idle_rotation(mode, BoxStyle::Japanese);
    let base_western = box_base_scale(BoxStyle::Western) * BOX_SCALE;
    let base_japanese = box_base_scale(BoxStyle::Japanese) * BOX_SCALE;

    for (index, style, mut transform, entrance) in &mut boxes {
        let slot_index = slot_of(index.0);
        let slot = slot_axis * (slot_index as f32);
        let (idle_rotation, base_scale, depth_nudge) = match style.0 {
            BoxStyle::Western => (idle_western, base_western, 0.0),
            BoxStyle::Japanese => (idle_japanese, base_japanese, JP_SPINE_ALIGN),
        };

        let Some(mut entrance) = entrance else {
            transform.translation = slot + entrance_offset(0.0) + Vec3::new(0.0, 0.0, depth_nudge);
            transform.rotation = idle_rotation;
            transform.scale = base_scale * ENTER_SCALE_FLOOR;
            continue;
        };

        entrance.progress = (entrance.progress + dt * ENTER_RATE).min(1.0 + entrance.delay);
        let t = (entrance.progress - entrance.delay).clamp(0.0, 1.0);
        let ef = EaseFunction::CubicOut.sample_clamped(t);
        let enter_scale = ENTER_SCALE_FLOOR + (1.0 - ENTER_SCALE_FLOOR) * ef;
        let enter_offset = entrance_offset(ef);

        if slot_index.abs_diff(selected_slot) > ANIMATED_WINDOW {
            transform.translation = slot + enter_offset + Vec3::new(0.0, 0.0, depth_nudge);
            transform.rotation = idle_rotation;
            transform.scale = base_scale * enter_scale;
            continue;
        }

        let selected = index.0 == state.selected;
        let sway = box_sway(slot_index, elapsed, selected);
        let wave = box_wave(slot_index, elapsed, selected);
        let mut target_translation;
        let target_rotation;
        let mut target_scale;

        if selected {
            let face = if state.box_flipped {
                Quat::from_rotation_y(PI) * flipped_box_rotation(style.0)
            } else {
                focused_box_rotation(style.0)
            };
            target_rotation = Quat::from_rotation_y(sway)
                * face
                * Quat::from_rotation_z((elapsed * 0.65).cos() * 0.008);
            let bob_z = (elapsed * 1.05).cos() * 0.003;
            target_translation = match mode {
                ViewMode::Row => Vec3::new(slot.x + BOX_FOCUS_X, wave, BOX_FOCUS_Z + bob_z),
                ViewMode::Ribbon => Vec3::new(slot.x, wave, BOX_FOCUS_Z + bob_z),
                ViewMode::Column => COLUMN_FOCUS - row_target + Vec3::new(0.0, wave, bob_z),
            };
            target_scale = base_scale * 1.065;
        } else {
            target_rotation = Quat::from_rotation_y(sway) * idle_rotation;
            target_translation = match mode {
                ViewMode::Row | ViewMode::Ribbon => Vec3::new(slot.x, wave, depth_nudge),
                ViewMode::Column => Vec3::new(wave, slot.y, depth_nudge),
            };
            target_scale = base_scale;
        }

        target_translation += enter_offset;
        target_scale *= enter_scale;

        transform
            .translation
            .smooth_nudge(&target_translation, APPROACH_RATE, dt);
        transform
            .rotation
            .smooth_nudge(&target_rotation, APPROACH_RATE, dt);
        transform
            .scale
            .smooth_nudge(&target_scale, APPROACH_RATE, dt);
    }
}

fn entrance_offset(ef: f32) -> Vec3 {
    Vec3::new(0.0, 0.0, -(1.0 - ef) * ENTER_DEPTH)
}

fn row_focus_target(mode: ViewMode, selected: usize) -> Vec3 {
    match mode {
        ViewMode::Row => Vec3::new(ROW_FOCUS_ANCHOR_X - selected as f32 * BOX_PITCH, 0.0, 0.0),
        ViewMode::Column => Vec3::new(
            COLUMN_X,
            COLUMN_TOP_Y + selected as f32 * COLUMN_PITCH,
            COLUMN_Z,
        ),
        ViewMode::Ribbon => Vec3::new(
            -(selected as f32) * BOX_PITCH * RIBBON_ROW_SCALE,
            RIBBON_ROW_Y,
            0.0,
        ),
    }
}

fn row_display_scale(mode: ViewMode) -> f32 {
    match mode {
        ViewMode::Ribbon => RIBBON_ROW_SCALE,
        _ => 1.0,
    }
}

fn mode_idle_rotation(mode: ViewMode, style: BoxStyle) -> Quat {
    match mode {
        ViewMode::Column => {
            let flatten = match style {
                BoxStyle::Western => -FRAC_PI_2,
                BoxStyle::Japanese => FRAC_PI_2,
            };
            Quat::from_rotation_x(COLUMN_SPINE_TILT)
                * Quat::from_rotation_z(flatten)
                * idle_box_rotation()
        }
        _ => idle_box_rotation(),
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

fn flipped_box_rotation(style: BoxStyle) -> Quat {
    let tilt = Quat::from_euler(EulerRot::XYZ, 0.018, 0.11, 0.026);
    match style {
        BoxStyle::Western => tilt,
        BoxStyle::Japanese => tilt * Quat::from_rotation_z(-FRAC_PI_2),
    }
}

fn box_base_scale(style: BoxStyle) -> Vec3 {
    match style {
        BoxStyle::Western => Vec3::ONE,
        BoxStyle::Japanese => Vec3::new(190.0 / 179.0, 107.0 / 128.0, 1.0),
    }
}
