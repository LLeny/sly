use bevy::prelude::*;
use bevy::tasks::IoTaskPool;

use core_model::Button;
use snes::Snes;

use crate::assets::{Models, UiSounds};
use crate::cartridge::{CartMotion, DetailCart, swap_detail_cart};
use crate::discovery::Library;
use crate::game::GameSession;
use crate::launch::GameLoadTask;
use crate::library;
use crate::state::{LauncherMode, LauncherState, ModeTimers};
use crate::ui::ui_sound;

const REPEAT_DELAY: f32 = 0.35;
const REPEAT_INTERVAL: f32 = 0.085;

const GAME_KEYMAP: [(KeyCode, Button); 12] = [
    (KeyCode::ArrowUp, Button::Up),
    (KeyCode::ArrowDown, Button::Down),
    (KeyCode::ArrowLeft, Button::Left),
    (KeyCode::ArrowRight, Button::Right),
    (KeyCode::KeyZ, Button::B),
    (KeyCode::KeyX, Button::A),
    (KeyCode::KeyA, Button::Y),
    (KeyCode::KeyS, Button::X),
    (KeyCode::KeyQ, Button::L),
    (KeyCode::KeyW, Button::R),
    (KeyCode::Enter, Button::Start),
    (KeyCode::Backspace, Button::Select),
];

pub fn game_input(keyboard: Res<ButtonInput<KeyCode>>, session: Option<ResMut<GameSession>>) {
    let Some(mut session) = session else {
        return;
    };

    for (key, button) in GAME_KEYMAP {
        if keyboard.just_pressed(key) {
            session.core.set_button(0, button, true);
        }
        if keyboard.just_released(key) {
            session.core.set_button(0, button, false);
        }
    }
}

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
    mut session: Option<ResMut<GameSession>>,
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
                let entry = &library.entries[state.selected];
                let path = entry.path.clone();
                let game = storage::GameId::new(core_model::System::Snes, entry.sha1.clone());
                let task = IoTaskPool::get().spawn(async move {
                    library::load_cartridge(&path).map(|cart| (Snes::new(cart), game))
                });
                commands.spawn(GameLoadTask(task));
                commands.spawn(ui_sound(ui_sounds.game_start.clone()));

                next_mode.set(LauncherMode::Launching);
                timers.launch_elapsed = 0.0;
                timers.return_active = false;
                state.status = "Booting selected cartridge".into();

                for mut cart in &mut carts {
                    if cart.index == state.selected {
                        cart.motion = CartMotion::Launching;
                    }
                }
            }
        }
        LauncherMode::Playing if keyboard.just_pressed(KeyCode::Escape) => {
            if let Some(session) = session.as_deref() {
                session.persist_save_ram();
            }

            next_mode.set(LauncherMode::Browsing);

            timers.return_elapsed = 0.0;
            timers.return_active = true;
        }
        LauncherMode::Playing => {
            if let Some(session) = session.as_deref_mut() {
                if keyboard.just_pressed(KeyCode::F5) {
                    session.save_state();
                }
                if keyboard.just_pressed(KeyCode::F8) {
                    session.load_state();
                }
            }
        }
        _ => {}
    }
}
