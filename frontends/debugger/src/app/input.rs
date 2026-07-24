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

        self.target.set_button(0, button, pressed);
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
            for button in self.pressed_buttons.drain(..) {
                self.target.set_button(0, button, false);
            }
        }
    }
}
