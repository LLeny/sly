use bevy::camera::visibility::RenderLayers;
use bevy::gltf::GltfMaterialName;
use bevy::pbr::ExtendedMaterial;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

use crate::materials::{CardboardExtension, CardboardMaterial};
use crate::shelf::{BoxEntrance, BoxIndex, RevealBurst};

#[derive(Component, Clone, Default)]
pub struct BoxTextures {
    pub front: Option<Handle<Image>>,
    pub back: Option<Handle<Image>>,
    pub spine: Option<Handle<Image>>,
    pub edges: Option<Handle<Image>>,
}

#[derive(Component, Clone, Default)]
pub struct CartLabelTexture(pub Option<Handle<Image>>);

#[derive(Component)]
pub struct CartridgeLabelSurface {
    pub cart_root: Entity,
}

#[derive(Component)]
pub struct SceneRenderLayer(pub usize);

#[allow(clippy::too_many_arguments)]
pub fn on_scene_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    layers: Query<&SceneRenderLayer>,
    box_indices: Query<&BoxIndex>,
    box_textures: Query<&BoxTextures>,
    cart_labels: Query<&CartLabelTexture>,
    primitives: Query<(&MeshMaterial3d<StandardMaterial>, &GltfMaterialName)>,
    entrances: Query<&BoxEntrance>,
    time: Res<Time>,
    mut burst: ResMut<RevealBurst>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cardboard_materials: ResMut<Assets<CardboardMaterial>>,
) {
    let Ok(layer) = layers.get(ready.entity) else {
        return;
    };
    let box_seed = box_indices
        .get(ready.entity)
        .map(|index| index.0 as f32)
        .unwrap_or_default();

    apply_scene_textures(
        &mut commands,
        &children,
        &primitives,
        ready.entity,
        box_textures
            .get(ready.entity)
            .ok()
            .map(|textures| (textures, box_seed)),
        cart_labels.get(ready.entity).ok(),
        Some(layer.0),
        &entrances,
        time.elapsed_secs(),
        &mut burst,
        &mut materials,
        &mut cardboard_materials,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn refresh_scene_materials(
    mut commands: Commands,
    children: Query<&Children>,
    boxes: Query<(Entity, &BoxIndex, &BoxTextures), Changed<BoxTextures>>,
    carts: Query<(Entity, &CartLabelTexture), Changed<CartLabelTexture>>,
    primitives: Query<(&MeshMaterial3d<StandardMaterial>, &GltfMaterialName)>,
    entrances: Query<&BoxEntrance>,
    time: Res<Time>,
    mut burst: ResMut<RevealBurst>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cardboard_materials: ResMut<Assets<CardboardMaterial>>,
) {
    for (root, index, textures) in &boxes {
        apply_scene_textures(
            &mut commands,
            &children,
            &primitives,
            root,
            Some((textures, index.0 as f32)),
            None,
            None,
            &entrances,
            time.elapsed_secs(),
            &mut burst,
            &mut materials,
            &mut cardboard_materials,
        );
    }
    for (root, label) in &carts {
        apply_scene_textures(
            &mut commands,
            &children,
            &primitives,
            root,
            None,
            Some(label),
            None,
            &entrances,
            time.elapsed_secs(),
            &mut burst,
            &mut materials,
            &mut cardboard_materials,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_scene_textures(
    commands: &mut Commands,
    children: &Query<&Children>,
    primitives: &Query<(&MeshMaterial3d<StandardMaterial>, &GltfMaterialName)>,
    root: Entity,
    box_art: Option<(&BoxTextures, f32)>,
    cart_label: Option<&CartLabelTexture>,
    render_layer: Option<usize>,
    entrances: &Query<&BoxEntrance>,
    now: f32,
    burst: &mut RevealBurst,
    materials: &mut Assets<StandardMaterial>,
    cardboard_materials: &mut Assets<CardboardMaterial>,
) {
    let mut any_ready = false;
    for descendant in children.iter_descendants(root) {
        if let Some(layer) = render_layer {
            commands
                .entity(descendant)
                .insert(RenderLayers::layer(layer));
        }
        let Ok((source, name)) = primitives.get(descendant) else {
            continue;
        };

        if matches!(name.0.as_str(), "PAL_Label" | "NTSC_Label") {
            commands
                .entity(descendant)
                .insert(CartridgeLabelSurface { cart_root: root });
        }

        let texture = texture_for_material(
            name.0.as_str(),
            box_art.map(|(textures, _)| textures),
            cart_label,
        );
        let Some(texture) = texture else {
            continue;
        };

        any_ready |= if name.0.starts_with("Western_Box_") {
            let seed = box_art.map(|(_, seed)| seed).unwrap_or_default();
            replace_cardboard_material(
                commands,
                descendant,
                source,
                texture,
                seed,
                materials,
                cardboard_materials,
            )
        } else {
            replace_material(commands, descendant, source, texture, materials)
        };
    }

    if any_ready {
        let mut cmd = commands.entity(root);
        cmd.insert(Visibility::Visible);

        if box_art.is_some() && !entrances.contains(root) {
            cmd.insert(BoxEntrance {
                progress: 0.0,
                delay: burst.next_delay(now),
            });
        }
    }
}

fn texture_for_material(
    name: &str,
    box_art: Option<&BoxTextures>,
    cart_label: Option<&CartLabelTexture>,
) -> Option<Handle<Image>> {
    match name {
        "Western_Box_Front" => box_art?.front.clone(),
        "Western_Box_Back" => {
            let art = box_art?;
            art.back.clone().or_else(|| art.front.clone())
        }
        "Western_Box_Spine" => {
            let art = box_art?;
            art.spine.clone().or_else(|| art.front.clone())
        }
        "Western_Box_Edges" => {
            let art = box_art?;
            art.edges
                .clone()
                .or_else(|| art.spine.clone())
                .or_else(|| art.front.clone())
        }
        "PAL_Label" | "NTSC_Label" => cart_label?.0.clone(),
        _ => None,
    }
}

fn replace_cardboard_material(
    commands: &mut Commands,
    entity: Entity,
    source: &MeshMaterial3d<StandardMaterial>,
    texture: Handle<Image>,
    seed: f32,
    materials: &Assets<StandardMaterial>,
    cardboard_materials: &mut Assets<CardboardMaterial>,
) -> bool {
    let Some(mut material) = materials.get(source.id()).cloned() else {
        return false;
    };
    material.base_color = Color::WHITE;
    material.base_color_texture = Some(texture);
    material.metallic = 0.0;
    material.perceptual_roughness = 0.77;
    material.reflectance = 0.24;
    material.clearcoat = 0.025;
    material.clearcoat_perceptual_roughness = 0.82;

    let material = cardboard_materials.add(ExtendedMaterial {
        base: material,
        extension: CardboardExtension {
            parameters: Vec4::new(0.00018, 0.12, 0.20, seed * 17.17 + 3.7),
        },
    });
    commands
        .entity(entity)
        .remove::<MeshMaterial3d<StandardMaterial>>()
        .insert(MeshMaterial3d(material));
    true
}

fn replace_material(
    commands: &mut Commands,
    entity: Entity,
    source: &MeshMaterial3d<StandardMaterial>,
    texture: Handle<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut material) = materials.get(source.id()).cloned() else {
        return false;
    };

    material.base_color = Color::WHITE;
    material.base_color_texture = Some(texture);
    commands
        .entity(entity)
        .insert(MeshMaterial3d(materials.add(material)));

    true
}
