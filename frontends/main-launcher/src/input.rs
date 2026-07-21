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

const REPEAT_DELAY: f32 = 0.35;
const REPEAT_INTERVAL: f32 = 0.085;

#[derive(Default)]
pub struct ScrollRepeat {
    timer: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn handle_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut commands: Commands,
    mode: Res<State<LauncherMode>>,
    mut next_mode: ResMut<NextState<LauncherMode>>,
    mut state: ResMut<LauncherState>,
    mut timers: ResMut<ModeTimers>,
    library: Res<Library>,
    models: Res<Models>,
    mut carts: Query<&mut DetailCart>,
    ui_sounds: Res<UiSounds>,
    mut repeat: Local<ScrollRepeat>,
) {
    match mode.get() {
        LauncherMode::Browsing => {
            if keyboard.just_pressed(KeyCode::KeyF) {
                state.box_flipped = !state.box_flipped;
            }

            if keyboard.just_pressed(KeyCode::Tab) {
                state.view_mode = state.view_mode.next();
                commands.spawn(ui_sound(ui_sounds.scroll.clone()));
            }

            if library.entries.is_empty() {
                return;
            }

            let axis = |pressed: &dyn Fn(KeyCode) -> bool| {
                i8::from(pressed(KeyCode::ArrowRight) || pressed(KeyCode::KeyD))
                    - i8::from(pressed(KeyCode::ArrowLeft) || pressed(KeyCode::KeyA))
            };
            let just = axis(&|k| keyboard.just_pressed(k));
            let held = axis(&|k| keyboard.pressed(k));

            let delta = if just != 0 {
                repeat.timer = REPEAT_DELAY;
                just
            } else if held != 0 {
                repeat.timer -= time.delta_secs();
                if repeat.timer <= 0.0 {
                    repeat.timer += REPEAT_INTERVAL;
                    held
                } else {
                    0
                }
            } else {
                0
            };

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
