use std::f32::consts::PI;

use bevy::prelude::*;
use bevy::world_serialization::WorldAsset;

use crate::assets::Models;
use crate::camera::CART_LAYER;
use crate::discovery::Library;
use crate::launch::LAUNCH_CLOSEUP_DURATION;
use crate::library::CartShell;
use crate::scene_textures::{CartLabelTexture, SceneRenderLayer};
use crate::state::ModeTimers;

const CART_CENTER_X: f32 = 0.0;
const CART_HOVER_Y: f32 = 0.025;
pub const CART_SWAP_DURATION: f32 = 0.52;
const CART_OUTGOING_DURATION: f32 = 0.38;

#[derive(Component)]
pub struct DetailCart {
    pub index: usize,
    pub shell: CartShell,
    pub motion: CartMotion,
}

#[derive(Clone, Copy)]
pub enum CartMotion {
    Incoming(f32),
    Hovering(f32),
    Outgoing(f32),
    Launching,
}

fn cart_scene(models: &Models, shell: CartShell) -> Handle<WorldAsset> {
    match shell {
        CartShell::Us => models.us_cart_scene.clone(),
        CartShell::JpPal => models.jp_pal_cart_scene.clone(),
    }
}

fn cart_display_scale(shell: CartShell) -> f32 {
    match shell {
        CartShell::JpPal => 2.50,
        CartShell::Us => 2.70,
    }
}

pub fn spawn_detail_cart(
    commands: &mut Commands,
    index: usize,
    library: &Library,
    models: &Models,
) {
    let Some(entry) = library.entries.get(index) else {
        return;
    };
    let label = library.art.get(index).and_then(|art| art.label.clone());
    let shell = entry.cart_shell();

    commands.spawn((
        WorldAssetRoot(cart_scene(models, shell)),
        Transform::default(),
        SceneRenderLayer(CART_LAYER),
        CartLabelTexture(label),
        Visibility::Hidden,
        DetailCart {
            index,
            shell,
            motion: CartMotion::Incoming(0.0),
        },
    ));
}

pub fn swap_detail_cart(
    commands: &mut Commands,
    carts: &mut Query<&mut DetailCart>,
    index: usize,
    library: &Library,
    models: &Models,
) {
    for mut cart in carts.iter_mut() {
        cart.motion = CartMotion::Outgoing(0.0);
    }
    spawn_detail_cart(commands, index, library, models);
}

pub fn animate_carts(
    time: Res<Time>,
    timers: Res<ModeTimers>,
    mut commands: Commands,
    mut carts: Query<(Entity, &mut DetailCart, &mut Transform)>,
) {
    let dt = time.delta_secs();

    for (entity, mut cart, mut transform) in &mut carts {
        let base_scale = cart_display_scale(cart.shell);
        let base_rotation = Quat::from_euler(EulerRot::XYZ, -0.08, 0.17, -0.025);

        match cart.motion {
            CartMotion::Incoming(age) => {
                let age = age + dt;
                let t = (age / CART_SWAP_DURATION).clamp(0.0, 1.0);
                let eased = EaseFunction::CubicOut.sample_clamped(t);
                let overshoot = (t * PI).sin() * 0.025;

                transform.translation = Vec3::new(
                    CART_CENTER_X,
                    0.28 + ((CART_HOVER_Y - 0.28) * eased) - overshoot,
                    0.0,
                );
                transform.rotation = base_rotation;
                transform.scale = Vec3::splat(base_scale);

                cart.motion = if t >= 1.0 {
                    CartMotion::Hovering(0.0)
                } else {
                    CartMotion::Incoming(age)
                };
            }
            CartMotion::Hovering(age) => {
                let age = age + dt;
                transform.translation =
                    Vec3::new(CART_CENTER_X, CART_HOVER_Y + (age * 1.7).sin() * 0.007, 0.0);
                transform.rotation =
                    base_rotation * Quat::from_rotation_y((age * 0.7).sin() * 0.035);
                transform.scale = Vec3::splat(base_scale);
                cart.motion = CartMotion::Hovering(age);
            }
            CartMotion::Outgoing(age) => {
                let age = age + dt;
                let t = (age / CART_OUTGOING_DURATION).clamp(0.0, 1.0);

                transform.translation.x = CART_CENTER_X;
                transform.translation.y = CART_HOVER_Y - t * t * 0.44;
                transform.rotation = base_rotation * Quat::from_rotation_z(t * 0.14);
                transform.scale = Vec3::splat(base_scale * (1.0 - t * 0.18));

                if t >= 1.0 {
                    commands.entity(entity).despawn();
                } else {
                    cart.motion = CartMotion::Outgoing(age);
                }
            }
            CartMotion::Launching => {
                let t = (timers.launch_elapsed / LAUNCH_CLOSEUP_DURATION).clamp(0.0, 1.0);
                let approach = EaseFunction::CubicOut.sample_clamped(t);
                let settle = (t * PI).sin() * 0.012;
                let close_rotation = Quat::from_euler(EulerRot::XYZ, -0.035, 0.045, 0.0);

                transform.translation =
                    Vec3::new(CART_CENTER_X, CART_HOVER_Y, approach * 0.095 + settle);
                transform.rotation = base_rotation.slerp(close_rotation, approach);
                transform.scale = Vec3::splat(base_scale * (1.0 + approach * 0.04));
            }
        }
    }
}
