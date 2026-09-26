use core_model::Button;
use gpui::Keystroke;

use super::*;

fn game_button_from_code(code: &str) -> Option<Button> {
    Some(match code {
        "ArrowUp" => Button::Up,
        "ArrowDown" => Button::Down,
        "ArrowLeft" => Button::Left,
        "ArrowRight" => Button::Right,
        "KeyZ" => Button::B,
        "KeyX" => Button::A,
        "KeyA" => Button::Y,
        "KeyS" => Button::X,
        "KeyQ" => Button::L,
        "KeyW" => Button::R,
        "Enter" => Button::Start,
        "Backspace" => Button::Select,
        _ => return None,
    })
}

fn game_button_from_key(key: &str) -> Option<Button> {
    Some(match key {
        "up" => Button::Up,
        "down" => Button::Down,
        "left" => Button::Left,
        "right" => Button::Right,
        "z" => Button::B,
        "x" => Button::A,
        "a" => Button::Y,
        "s" => Button::X,
        "q" => Button::L,
        "w" => Button::R,
        "enter" => Button::Start,
        "backspace" => Button::Select,
        _ => return None,
    })
}

fn savestate_key(key: &str) -> Option<SavestateKey> {
    Some(match key {
        "f5" => SavestateKey::Save,
        "f8" => SavestateKey::Load,
        "f6" => SavestateKey::PrevSlot,
        "f7" => SavestateKey::NextSlot,
        _ => return None,
    })
}

#[derive(Clone, Copy)]
pub(super) enum SavestateKey {
    Save,
    Load,
    PrevSlot,
    NextSlot,
}

impl Debugger {
    pub(super) fn game_key(
        &mut self,
        keystroke: &Keystroke,
        pressed: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if !self.game_focus.is_focused(window) {
            return;
        }

        if let Some(action) = savestate_key(&keystroke.key) {
            if pressed {
                let slot = self.active_slot();
                match action {
                    SavestateKey::Save => self.save_state(slot, cx),
                    SavestateKey::Load => self.load_state(slot, cx),
                    SavestateKey::PrevSlot => self.cycle_slot(false, cx),
                    SavestateKey::NextSlot => self.cycle_slot(true, cx),
                }
            }
            cx.stop_propagation();
            return;
        }

        let modifiers = keystroke.modifiers;
        if modifiers.platform || modifiers.control || modifiers.alt || modifiers.function {
            return;
        }

        let button = keystroke
            .code
            .as_deref()
            .and_then(game_button_from_code)
            .or_else(|| game_button_from_key(&keystroke.key));
        let Some(button) = button else {
            return;
        };

        let Some(controller) = self.target.controllers().into_iter().next() else {
            return;
        };

        if !controller.supports(button) {
            return;
        }

        self.target.set_button(controller.port, button, pressed);
        if pressed {
            if !self.pressed_buttons.contains(&button) {
                self.pressed_buttons.push(button);
            }
        } else {
            self.pressed_buttons.retain(|&held| held != button);
        }

        cx.stop_propagation();
    }

    pub(super) fn sync_game_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.focused(cx).is_none() {
            window.focus(&self.game_focus, cx);
        }

        if !self.game_focus.is_focused(window) && !self.pressed_buttons.is_empty() {
            if let Some(controller) = self.target.controllers().into_iter().next() {
                for button in self.pressed_buttons.drain(..) {
                    self.target.set_button(controller.port, button, false);
                }
            } else {
                self.pressed_buttons.clear();
            }
        }
    }
}
