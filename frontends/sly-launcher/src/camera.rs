use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Exposure, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::{TextureDimension, TextureFormat, TextureUsages};

use crate::touch::TouchControls;

pub const SHELF_EXPOSURE_EV100: f32 = 11.2;
pub const CART_EXPOSURE_EV100: f32 = 11.0;

pub const SHELF_LAYER: usize = 1;
pub const CART_LAYER: usize = 2;

#[derive(Component)]
pub struct DetailCamera;

pub struct ViewCameraSpec {
    pub layer: usize,
    pub transform: Transform,
    pub ev100: f32,
    pub clear: Color,
}

fn samples(controls: TouchControls) -> Msaa {
    if controls.0 { Msaa::Off } else { Msaa::Sample4 }
}

pub fn spawn_view_camera(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    controls: TouchControls,
    spec: ViewCameraSpec,
) -> (Entity, Handle<Image>) {
    let mut image = Image::new_uninit(
        default(),
        TextureDimension::D2,
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::all(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let target = images.add(image);

    let entity = commands
        .spawn((
            Camera3d::default(),
            samples(controls),
            Camera {
                order: -(spec.layer as isize),
                clear_color: spec.clear.into(),
                ..default()
            },
            Exposure { ev100: spec.ev100 },
            RenderTarget::Image(target.clone().into()),
            spec.transform,
            RenderLayers::layer(spec.layer),
        ))
        .id();
    (entity, target)
}

pub fn spawn_lights(commands: &mut Commands, controls: TouchControls) {
    let point_shadows = !controls.0;
    const LIGHT_RIGS: [(usize, f32, f32); 2] = [
        (SHELF_LAYER, 44_000.0, 1_750.0),
        (CART_LAYER, 32_000.0, 1_200.0),
    ];

    for (layer, point_intensity, dir_illuminance) in LIGHT_RIGS {
        commands.spawn((
            PointLight {
                intensity: point_intensity,
                range: 2.0,
                shadow_maps_enabled: point_shadows,
                ..default()
            },
            Transform::from_xyz(-0.26, 0.38, 0.46),
            RenderLayers::layer(layer),
        ));
        commands.spawn((
            DirectionalLight {
                illuminance: dir_illuminance,
                shadow_maps_enabled: true,
                ..default()
            },
            Transform::from_xyz(0.35, 0.55, 0.4).looking_at(Vec3::ZERO, Vec3::Y),
            RenderLayers::layer(layer),
        ));
    }
}
