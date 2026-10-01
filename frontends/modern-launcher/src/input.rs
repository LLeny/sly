use std::collections::HashMap;

use core_model::Button;
use gilrs::{Axis, EventType, Gilrs};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::settings::{Binding, Controls, GameButton, Settings, ViewMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Navigate(i32),
    Adjust(i32),
    Alphabet(i32),
    Console(i32),
    Consoles,
    Search,
    SearchBackspace,
    SearchClear,
    SearchFocus,
    ControllersChanged,
    Flip,
    CycleView,
    Launch,
    Back,
    Pause,
    QuickSave,
    QuickLoad,
    Settings,
    Fullscreen,
    Button(Button, bool),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActiveInput {
    #[default]
    Keyboard,
    Gamepad,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputContext {
    #[default]
    Browsing,
    Playing,
    Paused,
    Settings,
    Search,
    Transition,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Source {
    Key(KeyCode),
    PadButton(usize, gilrs::Button),
    PadAxis(usize, Axis, i8),
}

impl Source {
    fn belongs_to(self, id: usize) -> bool {
        match self {
            Self::Key(_) => false,
            Self::PadButton(pad, _) | Self::PadAxis(pad, _, _) => id == pad,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Held {
    Navigation(i32),
    Adjustment(i32),
    Button(Button),
    Other,
}

#[derive(Default)]
struct Repeat {
    direction: i32,
    adjustment: bool,
    next: f64,
}

impl Repeat {
    fn set(&mut self, direction: i32, adjustment: bool, now: f64, delay: f64) -> Option<Action> {
        let direction = direction.signum();

        if direction == self.direction && adjustment == self.adjustment {
            return None;
        }

        self.direction = direction;
        self.adjustment = adjustment;
        self.next = now + delay;
        (direction != 0).then(|| self.action())
    }

    fn action(&self) -> Action {
        if self.adjustment {
            Action::Adjust(self.direction)
        } else {
            Action::Navigate(self.direction)
        }
    }

    fn poll(&mut self, now: f64, interval: f64) -> Option<Action> {
        if self.direction == 0 || now < self.next {
            return None;
        }

        // Skip repeats missed during a stall instead of bursting them.
        self.next = now + interval;
        Some(self.action())
    }
}

pub struct Input {
    gilrs: Option<Gilrs>,
    pub gamepad_error: Option<String>,
    pub active: ActiveInput,
    controls: Controls,
    held: HashMap<Source, Held>,
    repeat: Repeat,
    focused: bool,
    context: InputContext,
    layout: ViewMode,
    connected: usize,
    remap: Option<GameButton>,
    remapped: Option<(ActiveInput, String, GameButton)>,
}

impl Input {
    pub fn new(settings: &Settings) -> Self {
        let (gilrs, gamepad_error, connected) = match Gilrs::new() {
            Ok(gilrs) => {
                let connected = gilrs.gamepads().count();
                (Some(gilrs), None, connected)
            }
            Err(error) => (None, Some(format!("Gamepad input unavailable: {error}")), 0),
        };

        Self {
            gilrs,
            gamepad_error,
            active: ActiveInput::Keyboard,
            controls: settings.controls.clone(),
            held: HashMap::new(),
            repeat: Repeat::default(),
            focused: true,
            context: InputContext::Browsing,
            layout: settings.layout,
            connected,
            remap: None,
            remapped: None,
        }
    }

    /// Callers must release held game buttons themselves.
    pub fn configure(&mut self, settings: &Settings) {
        self.clear();
        self.controls = settings.controls.clone();
    }

    pub fn connected_gamepads(&self) -> usize {
        self.connected
    }

    pub fn begin_remap(&mut self, button: GameButton) {
        self.remap = Some(button);
        self.remapped = None;
        self.repeat = Repeat::default();
    }

    pub fn cancel_remap(&mut self) {
        self.remap = None;
        self.remapped = None;
    }

    pub fn take_remap(&mut self) -> Option<(ActiveInput, String, GameButton)> {
        self.remapped.take()
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;

        if !focused {
            self.clear();
        }
    }

    pub fn clear(&mut self) -> Vec<Action> {
        let releases = Button::ALL
            .into_iter()
            .filter(|button| {
                self.held
                    .values()
                    .any(|held| *held == Held::Button(*button))
            })
            .map(|button| Action::Button(button, false))
            .collect();

        self.held.clear();
        self.repeat = Repeat::default();
        releases
    }

    fn set_context(&mut self, context: InputContext, layout: ViewMode) -> Vec<Action> {
        if context == self.context && layout == self.layout {
            return Vec::new();
        }

        let releases = self.clear();
        self.context = context;
        self.layout = layout;
        releases
    }

    pub fn keyboard(
        &mut self,
        event: &KeyEvent,
        context: InputContext,
        layout: ViewMode,
        now: f64,
    ) -> Vec<Action> {
        let mut actions = self.set_context(context, layout);

        let PhysicalKey::Code(code) = event.physical_key else {
            return actions;
        };

        self.keyboard_key(code, event.state, event.repeat, now, &mut actions);
        actions
    }

    fn keyboard_key(
        &mut self,
        code: KeyCode,
        state: ElementState,
        repeat: bool,
        now: f64,
        actions: &mut Vec<Action>,
    ) {
        let source = Source::Key(code);

        if state == ElementState::Released {
            self.release(source, now, actions);
            return;
        }

        if !self.focused {
            return;
        }

        // Backspace in search follows the OS key repeat.
        if self.context == InputContext::Search && code == KeyCode::Backspace {
            self.active = ActiveInput::Keyboard;
            self.held.insert(source, Held::Other);
            actions.push(Action::SearchBackspace);
            return;
        }

        if repeat || self.held.contains_key(&source) {
            return;
        }

        self.active = ActiveInput::Keyboard;
        let name = format!("{code:?}");

        if code == KeyCode::Escape {
            self.cancel_remap();
            self.held.insert(source, Held::Other);
            actions.push(Action::Back);
            return;
        }

        if self.context == InputContext::Search {
            if let Some(binding) = arrow_binding(code) {
                self.press_binding(source, binding, now, actions);
            } else {
                self.held.insert(source, Held::Other);

                if let Some(action) = match code {
                    KeyCode::Enter | KeyCode::NumpadEnter => Some(Action::Launch),
                    KeyCode::Tab => Some(Action::SearchFocus),
                    KeyCode::Delete => Some(Action::SearchClear),
                    KeyCode::F11 => Some(Action::Fullscreen),
                    _ => None,
                } {
                    actions.push(action);
                }
            }

            // Text arrives through IME events instead.
            return;
        }

        if self.capture(source, ActiveInput::Keyboard, &name) {
            return;
        }

        if self.context == InputContext::Playing
            && let Some(button) = self.controls.game_key(&name)
        {
            self.press_game(source, button, actions);
            return;
        }

        let binding = if matches!(self.context, InputContext::Settings | InputContext::Paused) {
            arrow_binding(code)
                .or(matches!(code, KeyCode::Enter | KeyCode::Space).then_some(Binding::Launch))
        } else {
            None
        }
        .or_else(|| self.controls.keyboard_binding(&name));

        if let Some(binding) = binding {
            self.press_binding(source, binding, now, actions);
        }
    }

    pub fn poll(&mut self, now: f64, context: InputContext, layout: ViewMode) -> Vec<Action> {
        let mut actions = self.set_context(context, layout);

        while let Some(event) = self.gilrs.as_mut().and_then(Gilrs::next_event) {
            let id = usize::from(event.id);

            match event.event {
                EventType::Connected => {
                    self.connected = self.count_gamepads();
                    actions.push(Action::ControllersChanged);
                }
                EventType::Disconnected => {
                    self.disconnect(id, now, &mut actions);
                    self.connected = self.count_gamepads();

                    if self.connected == 0 && self.active == ActiveInput::Gamepad {
                        self.active = ActiveInput::Keyboard;
                    }

                    actions.push(Action::ControllersChanged);
                }
                EventType::ButtonReleased(button, _) => {
                    self.release(Source::PadButton(id, button), now, &mut actions);
                }
                EventType::ButtonPressed(button, _) if self.focused => {
                    self.active = ActiveInput::Gamepad;
                    let source = Source::PadButton(id, button);

                    if self.held.contains_key(&source) {
                        continue;
                    }

                    let name = format!("{button:?}");

                    if (button == gilrs::Button::East && context != InputContext::Playing)
                        || (button == gilrs::Button::Mode && self.remap.is_some())
                    {
                        self.cancel_remap();
                        self.held.insert(source, Held::Other);
                        actions.push(Action::Back);
                        continue;
                    }

                    if self.capture(source, ActiveInput::Gamepad, &name) {
                        continue;
                    }

                    if context == InputContext::Search {
                        self.press_search_button(source, button, now, &mut actions);
                        continue;
                    }

                    if button == gilrs::Button::Mode {
                        self.held.insert(source, Held::Other);
                        actions.push(Action::Pause);
                        continue;
                    }

                    if context == InputContext::Playing
                        && let Some(button) = self.controls.game_pad(&name)
                    {
                        self.press_game(source, button, &mut actions);
                    } else {
                        let menu_binding =
                            if matches!(context, InputContext::Settings | InputContext::Paused) {
                                dpad_binding(button).or(match button {
                                    gilrs::Button::South => Some(Binding::Launch),
                                    gilrs::Button::Start
                                        if self.controls.gamepad_binding(&name)
                                            == Some(Binding::Consoles) =>
                                    {
                                        Some(Binding::Pause)
                                    }
                                    _ => None,
                                })
                            } else {
                                None
                            };

                        if let Some(binding) =
                            menu_binding.or_else(|| self.controls.gamepad_binding(&name))
                        {
                            self.press_binding(source, binding, now, &mut actions);
                        }
                    }
                }
                EventType::AxisChanged(axis @ (Axis::LeftStickX | Axis::LeftStickY), value, _)
                    if self.focused && self.remap.is_none() =>
                {
                    self.axis(id, axis, value, now, &mut actions);
                }
                _ => {}
            }
        }

        if self.focused
            && matches!(
                context,
                InputContext::Browsing
                    | InputContext::Settings
                    | InputContext::Paused
                    | InputContext::Search
            )
            && self.remap.is_none()
            && let Some(action) = self.repeat.poll(now, self.controls.repeat_interval)
        {
            actions.push(action);
        }

        actions
    }

    fn count_gamepads(&self) -> usize {
        self.gilrs
            .as_ref()
            .map_or(0, |gilrs| gilrs.gamepads().count())
    }

    fn capture(&mut self, source: Source, device: ActiveInput, name: &str) -> bool {
        let Some(button) = self.remap else {
            return false;
        };

        if self.held.contains_key(&source) {
            return true;
        }

        self.remap = None;
        self.remapped = Some((device, name.to_owned(), button));
        self.held.insert(source, Held::Other);
        true
    }

    fn press_search_button(
        &mut self,
        source: Source,
        button: gilrs::Button,
        now: f64,
        actions: &mut Vec<Action>,
    ) {
        if let Some(binding) = dpad_binding(button) {
            self.press_binding(source, binding, now, actions);
        } else if self.held.insert(source, Held::Other).is_none()
            && let Some(action) = match button {
                gilrs::Button::South => Some(Action::Launch),
                gilrs::Button::East => Some(Action::Back),
                gilrs::Button::West => Some(Action::SearchBackspace),
                gilrs::Button::North => Some(Action::SearchFocus),
                gilrs::Button::Select => Some(Action::SearchClear),
                _ => None,
            }
        {
            actions.push(action);
        }
    }

    fn press_game(&mut self, source: Source, button: Button, actions: &mut Vec<Action>) {
        if self.held.contains_key(&source) {
            return;
        }

        let already_pressed = self.held.values().any(|held| *held == Held::Button(button));
        self.held.insert(source, Held::Button(button));

        if !already_pressed {
            actions.push(Action::Button(button, true));
        }
    }

    fn press_binding(
        &mut self,
        source: Source,
        binding: Binding,
        now: f64,
        actions: &mut Vec<Action>,
    ) {
        if self.held.contains_key(&source) {
            return;
        }

        let action = binding_action(binding, self.context, self.layout);

        match action {
            Some(Action::Navigate(direction)) => {
                self.held.insert(source, Held::Navigation(direction));
                self.refresh_repeat(now, actions);
            }
            Some(Action::Adjust(direction)) => {
                self.held.insert(source, Held::Adjustment(direction));
                self.refresh_repeat(now, actions);
            }
            Some(action) => {
                self.held.insert(source, Held::Other);
                actions.push(action);
            }
            None => {
                self.held.insert(source, Held::Other);
            }
        }
    }

    fn release(&mut self, source: Source, now: f64, actions: &mut Vec<Action>) {
        match self.held.remove(&source) {
            Some(Held::Button(button)) => {
                if !self.held.values().any(|held| *held == Held::Button(button)) {
                    actions.push(Action::Button(button, false));
                }
            }
            Some(Held::Navigation(_) | Held::Adjustment(_)) => self.refresh_repeat(now, actions),
            _ => {}
        }
    }

    fn refresh_repeat(&mut self, now: f64, actions: &mut Vec<Action>) {
        let direction: i32 = self
            .held
            .values()
            .filter_map(|held| match held {
                Held::Navigation(direction) => Some(*direction),
                _ => None,
            })
            .sum();

        let adjustment: i32 = self
            .held
            .values()
            .filter_map(|held| match held {
                Held::Adjustment(direction) => Some(*direction),
                _ => None,
            })
            .sum();

        let (direction, is_adjustment) = if direction != 0 {
            (direction, false)
        } else {
            (adjustment, true)
        };

        if let Some(action) =
            self.repeat
                .set(direction, is_adjustment, now, self.controls.repeat_delay)
        {
            actions.push(action);
        }
    }

    fn disconnect(&mut self, id: usize, now: f64, actions: &mut Vec<Action>) {
        let sources: Vec<_> = self
            .held
            .keys()
            .copied()
            .filter(|source| source.belongs_to(id))
            .collect();

        // Remove every held source first so opposite directions don't cause spurious steps.
        let mut released = Vec::new();

        for source in sources {
            if let Some(Held::Button(button)) = self.held.remove(&source) {
                released.push(button);
            }
        }

        for button in Button::ALL {
            if released.contains(&button)
                && !self.held.values().any(|held| *held == Held::Button(button))
            {
                actions.push(Action::Button(button, false));
            }
        }

        self.refresh_repeat(now, actions);
    }

    fn axis(&mut self, id: usize, axis: Axis, value: f32, now: f64, actions: &mut Vec<Action>) {
        if !value.is_finite() {
            return;
        }

        for sign in [-1_i8, 1_i8] {
            let source = Source::PadAxis(id, axis, sign);
            let was_held = self.held.contains_key(&source);
            let threshold = self.controls.stick_deadzone * if was_held { 0.75 } else { 1.0 };
            let pressed = value * f32::from(sign) > threshold;

            if !pressed {
                self.release(source, now, actions);
                continue;
            }

            if was_held {
                continue;
            }

            self.active = ActiveInput::Gamepad;

            // Gilrs uses positive Y for the upward direction.
            let (binding, button) = match (axis, sign) {
                (Axis::LeftStickX, -1) => (Binding::Left, Button::Left),
                (Axis::LeftStickX, _) => (Binding::Right, Button::Right),
                (Axis::LeftStickY, -1) => (Binding::Down, Button::Down),
                _ => (Binding::Up, Button::Up),
            };

            if self.context == InputContext::Playing {
                self.press_game(source, button, actions);
            } else {
                self.press_binding(source, binding, now, actions);
            }
        }
    }

    pub fn control_hints(&self, compact: bool) -> Vec<(String, &'static str)> {
        let mut prompts = Vec::new();

        if !compact {
            let vertical = self.layout == ViewMode::Column;

            let directions = if vertical {
                [Binding::Up, Binding::Down]
            } else {
                [Binding::Left, Binding::Right]
            };

            let controls: Vec<_> = directions
                .into_iter()
                .filter_map(|binding| self.control_label(binding))
                .collect();

            if !controls.is_empty() {
                let pair = |prefix| {
                    controls.len() == 2
                        && controls.iter().all(|control| control.starts_with(prefix))
                };

                let control = if pair("DPad") {
                    "D-pad".into()
                } else if pair("Arrow") {
                    if vertical {
                        "Vertical arrows"
                    } else {
                        "Arrows"
                    }
                    .into()
                } else {
                    controls.join(" / ")
                };

                prompts.push((control, "Browse"));
            }
        }

        for (binding, action) in [
            (Binding::Launch, "Play"),
            (Binding::Flip, "Flip"),
            (Binding::CycleView, "View"),
            (Binding::Consoles, "Consoles"),
            (Binding::Search, "Search"),
            (Binding::Settings, "Settings"),
        ] {
            if compact
                && !matches!(
                    binding,
                    Binding::Launch | Binding::Settings | Binding::Consoles | Binding::Search
                )
            {
                continue;
            }

            if let Some(label) = self.control_label(binding) {
                prompts.push((label, action));
            }
        }

        prompts
    }

    pub fn search_hints(&self) -> Vec<(String, &'static str)> {
        let gamepad = self.active == ActiveInput::Gamepad;

        [
            ("Enter", "South", "Pick"),
            ("Tab", "North", "Keyboard / results"),
            ("Backspace", "West", "Delete"),
            ("Delete", "Select", "Clear"),
            ("Escape", "East", "Back"),
        ]
        .map(|(key, button, action)| (if gamepad { button } else { key }.to_owned(), action))
        .into_iter()
        .collect()
    }

    pub fn console_shortcuts(&self) -> String {
        let previous = self.control_label(Binding::PreviousConsole);
        let next = self.control_label(Binding::NextConsole);

        let display = |control: String| match control.as_str() {
            "LeftTrigger2" => "LT".into(),
            "RightTrigger2" => "RT".into(),
            "PageUp" => "Page Up".into(),
            "PageDown" => "Page Down".into(),
            _ => control,
        };

        match (previous, next) {
            (Some(previous), Some(next)) => format!(
                "{} / {}: previous / next console",
                display(previous),
                display(next)
            ),
            _ => "Choose a console to browse its games".into(),
        }
    }

    fn control_label(&self, binding: Binding) -> Option<String> {
        let (overrides, defaults) = match self.active {
            ActiveInput::Gamepad => (&self.controls.gamepad, &Controls::DEFAULT_PAD[..]),
            ActiveInput::Keyboard => (&self.controls.keyboard, &Controls::DEFAULT_KEYS[..]),
        };

        let name = overrides
            .iter()
            .find_map(|(name, candidate)| (*candidate == binding).then_some(name.as_str()))
            .or_else(|| {
                defaults.iter().find_map(|&(name, default)| {
                    let unchanged = overrides.get(name).is_none_or(|bound| *bound == default);
                    (default == binding && unchanged).then_some(name)
                })
            });

        name.map(|name| name.strip_prefix("Key").unwrap_or(name).to_owned())
    }
}

fn arrow_binding(code: KeyCode) -> Option<Binding> {
    match code {
        KeyCode::ArrowUp => Some(Binding::Up),
        KeyCode::ArrowDown => Some(Binding::Down),
        KeyCode::ArrowLeft => Some(Binding::Left),
        KeyCode::ArrowRight => Some(Binding::Right),
        _ => None,
    }
}

fn dpad_binding(button: gilrs::Button) -> Option<Binding> {
    match button {
        gilrs::Button::DPadUp => Some(Binding::Up),
        gilrs::Button::DPadDown => Some(Binding::Down),
        gilrs::Button::DPadLeft => Some(Binding::Left),
        gilrs::Button::DPadRight => Some(Binding::Right),
        _ => None,
    }
}

fn binding_action(binding: Binding, context: InputContext, layout: ViewMode) -> Option<Action> {
    use Binding as B;

    if binding == B::Disabled {
        return None;
    }

    if binding == B::Back {
        return Some(Action::Back);
    }

    if binding == B::Fullscreen {
        return Some(Action::Fullscreen);
    }

    if context == InputContext::Transition {
        return None;
    }

    if context == InputContext::Search {
        return Some(match binding {
            B::Up => Action::Navigate(-1),
            B::Down => Action::Navigate(1),
            B::Left => Action::Adjust(-1),
            B::Right => Action::Adjust(1),
            B::Launch => Action::Launch,
            _ => return None,
        });
    }

    if binding == B::Settings {
        return Some(Action::Settings);
    }

    match context {
        InputContext::Settings | InputContext::Paused => Some(match binding {
            B::PreviousConsole => Action::Console(-1),
            B::NextConsole => Action::Console(1),
            B::Up | B::Previous => Action::Navigate(-1),
            B::Down | B::Next => Action::Navigate(1),
            B::Left => Action::Adjust(-1),
            B::Right => Action::Adjust(1),
            B::Launch => Action::Launch,
            B::Pause => Action::Pause,
            B::Consoles => Action::Consoles,
            B::QuickSave => Action::QuickSave,
            B::QuickLoad => Action::QuickLoad,
            _ => return None,
        }),
        InputContext::Browsing => Some(match binding {
            B::PreviousLetter => Action::Alphabet(-1),
            B::NextLetter => Action::Alphabet(1),
            B::PreviousConsole => Action::Console(-1),
            B::NextConsole => Action::Console(1),
            B::Consoles => Action::Consoles,
            B::Search => Action::Search,
            B::Previous => Action::Navigate(-1),
            B::Next => Action::Navigate(1),
            B::Up if layout.is_vertical() => Action::Navigate(-1),
            B::Down if layout.is_vertical() => Action::Navigate(1),
            B::Left if !layout.is_vertical() => Action::Navigate(-1),
            B::Right if !layout.is_vertical() => Action::Navigate(1),
            B::Flip => Action::Flip,
            B::CycleView => Action::CycleView,
            B::Launch => Action::Launch,
            _ => return None,
        }),
        InputContext::Playing => Some(match binding {
            B::Pause => Action::Pause,
            B::QuickSave => Action::QuickSave,
            B::QuickLoad => Action::QuickLoad,
            _ => return None,
        }),
        _ => None,
    }
}
