use bevy::prelude::*;

use core_model::Button;

use crate::assets::{Models, UiSounds};
use crate::cartridge::{DetailCart, swap_detail_cart};
use crate::discovery::Library;
use crate::game::GameSession;
use crate::launch;
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

pub fn select(
    index: usize,
    commands: &mut Commands,
    state: &mut LauncherState,
    library: &Library,
    carts: &mut Query<&mut DetailCart>,
    models: &Models,
    ui_sounds: &UiSounds,
) {
    if index >= library.entries.len() || index == state.selected {
        return;
    }

    state.selected = index;
    state.box_flipped = false;

    swap_detail_cart(commands, carts, index, library, models);
    commands.spawn(ui_sound(ui_sounds.scroll.clone()));
}

pub fn step_selection(
    delta: i8,
    commands: &mut Commands,
    state: &mut LauncherState,
    library: &Library,
    carts: &mut Query<&mut DetailCart>,
    models: &Models,
    ui_sounds: &UiSounds,
) {
    if library.entries.is_empty() {
        return;
    }

    let target = if delta > 0 {
        (state.selected + 1).min(library.entries.len() - 1)
    } else {
        state.selected.saturating_sub(1)
    };

    select(target, commands, state, library, carts, models, ui_sounds);
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

            let (forward, back) = if state.view_mode.is_vertical() {
                (
                    [KeyCode::ArrowDown, KeyCode::KeyS],
                    [KeyCode::ArrowUp, KeyCode::KeyW],
                )
            } else {
                (
                    [KeyCode::ArrowRight, KeyCode::KeyD],
                    [KeyCode::ArrowLeft, KeyCode::KeyA],
                )
            };
            let axis = |pressed: &dyn Fn(KeyCode) -> bool| {
                i8::from(forward.iter().any(|&key| pressed(key)))
                    - i8::from(back.iter().any(|&key| pressed(key)))
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
                step_selection(
                    delta,
                    &mut commands,
                    &mut state,
                    &library,
                    &mut carts,
                    &models,
                    &ui_sounds,
                );
            }

            if keyboard.just_pressed(KeyCode::Enter) {
                launch::begin_launch(
                    &mut commands,
                    &mut state,
                    &mut timers,
                    &mut next_mode,
                    &library,
                    &mut carts,
                    &ui_sounds,
                );
            }
        }
        LauncherMode::Playing if keyboard.just_pressed(KeyCode::Escape) => {
            launch::return_to_library(session.as_deref(), &mut next_mode, &mut timers);
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
