use bevy::prelude::*;
use bevy::tasks::IoTaskPool;

use snes::Snes;

use crate::assets::{Models, UiSounds};
use crate::cartridge::{CartMotion, DetailCart, swap_detail_cart};
use crate::discovery::Library;
use crate::launch::GameLoadTask;
use crate::library;
use crate::state::{LauncherMode, LauncherState, ModeTimers};
use crate::ui::ui_sound;

#[allow(clippy::too_many_arguments)]
pub fn handle_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mode: Res<State<LauncherMode>>,
    mut next_mode: ResMut<NextState<LauncherMode>>,
    mut state: ResMut<LauncherState>,
    mut timers: ResMut<ModeTimers>,
    library: Res<Library>,
    models: Res<Models>,
    mut carts: Query<&mut DetailCart>,
    ui_sounds: Res<UiSounds>,
) {
    match mode.get() {
        LauncherMode::Browsing => {
            if keyboard.just_pressed(KeyCode::KeyF) {
                state.box_flipped = !state.box_flipped;
            }

            if library.entries.is_empty() {
                return;
            }

            let delta = i8::from(
                keyboard.just_pressed(KeyCode::ArrowRight) || keyboard.just_pressed(KeyCode::KeyD),
            ) - i8::from(
                keyboard.just_pressed(KeyCode::ArrowLeft) || keyboard.just_pressed(KeyCode::KeyA),
            );
            if delta != 0 {
                let target = if delta > 0 {
                    (state.selected + 1).min(library.entries.len() - 1)
                } else {
                    state.selected.saturating_sub(1)
                };

                if target != state.selected {
                    state.selected = target;
                    state.box_flipped = false;

                    swap_detail_cart(&mut commands, &mut carts, target, &library, &models);
                    commands.spawn(ui_sound(ui_sounds.scroll.clone()));
                }
            }

            if keyboard.just_pressed(KeyCode::Enter) {
                let path = library.entries[state.selected].path.clone();
                let task = IoTaskPool::get()
                    .spawn(async move { library::load_cartridge(&path).map(Snes::new) });
                commands.spawn(GameLoadTask(task));
                commands.spawn(ui_sound(ui_sounds.game_start.clone()));

                next_mode.set(LauncherMode::Launching);
                timers.launch_elapsed = 0.0;
                state.status = "Booting selected cartridge".into();

                for mut cart in &mut carts {
                    if cart.index == state.selected {
                        cart.motion = CartMotion::Launching;
                    }
                }
            }
        }
        LauncherMode::Playing if keyboard.just_pressed(KeyCode::Escape) => {
            next_mode.set(LauncherMode::Browsing);
        }
        _ => {}
    }
}
