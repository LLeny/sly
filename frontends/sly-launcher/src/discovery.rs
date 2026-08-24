use bevy::prelude::*;
use bevy::tasks::futures::check_ready;
use bevy::tasks::{IoTaskPool, Task};

use artwork::layle as artwork;

use crate::art_processing::{ProcessedArtwork, process_artwork};
use crate::assets::Models;
use crate::cartridge::{DetailCart, spawn_detail_cart};
use crate::config::Config;
use crate::library::{self, LibraryEntry};
use crate::scene_textures::{BoxTextures, CartLabelTexture};
use crate::shelf::{RowRoot, spawn_box};
use crate::state::{LauncherMode, LauncherState};

fn empty_library_message() -> String {
    let extensions = core_model::System::Snes
        .rom_extensions()
        .iter()
        .map(|extension| format!(".{extension}"))
        .collect::<Vec<_>>()
        .join(" or ");

    #[cfg(target_os = "ios")]
    return format!("Copy {extensions} files into the Sly folder in Files");

    #[cfg(not(target_os = "ios"))]
    format!("No valid {extensions} files found")
}

#[derive(Resource, Default)]
pub struct Library {
    pub entries: Vec<LibraryEntry>,
    pub art: Vec<CartArt>,
    pub box_entities: Vec<Entity>,
}

impl Library {
    pub fn first_with_letter(&self, letter: usize) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| title_alphabet_index(&entry.title) == letter)
    }
}

pub fn title_alphabet_index(title: &str) -> usize {
    title
        .chars()
        .find(|character| character.is_ascii_alphabetic())
        .map(|character| (character.to_ascii_uppercase() as u8 - b'A') as usize)
        .unwrap_or(0)
}

#[derive(Clone, Default)]
pub struct CartArt {
    pub label: Option<Handle<Image>>,
    pub dominant_color: Option<Vec3>,
}

#[derive(Component)]
pub struct DiscoveryTask(Task<Result<Vec<LibraryEntry>, String>>);

#[derive(Component)]
pub struct ArtworkTask {
    index: usize,
    task: Task<Result<ProcessedArtwork, String>>,
}

pub fn start_discovery(mut commands: Commands, config: Res<Config>) {
    let roots = config.library.roms.clone();
    info!("config: {}, scanning {roots:?}", config.source);

    let task = IoTaskPool::get().spawn(async move { library::discover(&roots) });
    commands.spawn(DiscoveryTask(task));
}

pub fn poll_discovery(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut DiscoveryTask)>,
    mut state: ResMut<LauncherState>,
    mut next_mode: ResMut<NextState<LauncherMode>>,
    mut library: ResMut<Library>,
    models: Res<Models>,
    row: Single<Entity, With<RowRoot>>,
) {
    for (entity, mut task) in &mut tasks {
        let Some(result) = check_ready(&mut task.0) else {
            continue;
        };
        commands.entity(entity).despawn();

        next_mode.set(LauncherMode::Browsing);
        match result {
            Ok(entries) if !entries.is_empty() => {
                let library = &mut *library;
                library.entries = entries;
                library.art = vec![CartArt::default(); library.entries.len()];
                library.box_entities.clear();

                for (index, entry) in library.entries.iter().enumerate() {
                    let box_entity =
                        spawn_box(&mut commands, *row, index, entry.box_style(), &models);
                    library.box_entities.push(box_entity);

                    let sha1 = entry.sha1.clone();
                    let region = entry.region;
                    let task = IoTaskPool::get().spawn(async move {
                        artwork::fetch_bundle(&sha1, region)
                            .map(|bundle| process_artwork(bundle, region))
                    });
                    commands.spawn(ArtworkTask { index, task });
                }

                spawn_detail_cart(&mut commands, 0, library, &models);
                state.status = format!("{} games indexed", library.entries.len());
            }
            Ok(_) => state.status = empty_library_message(),
            Err(err) => state.status = err,
        }
    }
}

pub fn poll_artwork(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut ArtworkTask)>,
    mut library: ResMut<Library>,
    mut images: ResMut<Assets<Image>>,
    mut boxes: Query<&mut BoxTextures>,
    mut carts: Query<(&DetailCart, &mut CartLabelTexture)>,
) {
    for (task_entity, mut task) in &mut tasks {
        let Some(result) = check_ready(&mut task.task) else {
            continue;
        };
        commands.entity(task_entity).despawn();
        let Ok(processed) = result else {
            continue;
        };

        let textures = BoxTextures {
            front: processed.front.map(|image| images.add(image)),
            back: processed.back.map(|image| images.add(image)),
            spine: processed.spine.map(|image| images.add(image)),
            edges: processed.edges.map(|image| images.add(image)),
        };
        let label = processed.label.map(|image| images.add(image));

        if let Some(entry) = library.entries.get_mut(task.index)
            && !entry.meta.has_normalized_title()
        {
            entry.title = processed.title;
        }
        if let Some(slot) = library.art.get_mut(task.index) {
            *slot = CartArt {
                label: label.clone(),
                dominant_color: processed.dominant_color,
            };
        }

        if let Some(&box_entity) = library.box_entities.get(task.index)
            && let Ok(mut box_textures) = boxes.get_mut(box_entity)
        {
            *box_textures = textures;
        }

        for (cart, mut cart_label) in &mut carts {
            if cart.index == task.index {
                cart_label.0 = label.clone();
            }
        }
    }
}
