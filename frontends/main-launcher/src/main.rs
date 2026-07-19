mod artwork;

use bevy::asset::RenderAssetUsages;
use bevy::gltf::GltfMaterialName;
use bevy::image::{CompressedImageFormats, ImageFormat, ImageSampler, ImageType};
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct LabelTexture(Handle<Image>);

#[derive(Component)]
#[allow(dead_code)]
struct Cartridge {
    console: String,
    rom: String,
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 0.2).looking_at(Vec3::ZERO, Vec3::Y),
        MainCamera,
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.3, 0.5, 0.3).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn add_roms(mut commands: Commands, assets: Res<AssetServer>, mut images: ResMut<Assets<Image>>) {
    let console = "snes";
    let rom = "private/Super Mario World (USA).sfc";

    let model = assets.load(GltfAssetLabel::Scene(0).from_asset("snes-pal.glb"));
    let label = artwork::fetch_label(console, rom);
    let image = Image::from_buffer(
        &label,
        ImageType::Format(ImageFormat::Png),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::linear(),
        RenderAssetUsages::default(),
    )
    .expect("Failed to create image from label bytes");

    let label = images.add(image);

    commands.spawn((
        WorldAssetRoot(model),
        Cartridge {
            console: console.to_string(),
            rom: rom.to_string(),
        },
        LabelTexture(label),
    ));
}

fn apply_cartridge_label(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    labels: Query<&LabelTexture>,
    primitives: Query<(&MeshMaterial3d<StandardMaterial>, &GltfMaterialName)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(label) = labels.get(ready.entity) else {
        return;
    };

    for descendant in children.iter_descendants(ready.entity) {
        let Ok((source, name)) = primitives.get(descendant) else {
            continue;
        };

        if name.0 != "PAL_Label" {
            continue;
        }

        let Some(mut material) = materials.get(source.id()).cloned() else {
            continue;
        };

        material.base_color = Color::WHITE;
        material.base_color_texture = Some(label.0.clone());

        commands
            .entity(descendant)
            .insert(MeshMaterial3d(materials.add(material)));
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: "models".into(),
            ..default()
        }))
        .add_systems(Startup, (setup_camera, add_roms))
        .add_observer(apply_cartridge_label)
        .run();
}
