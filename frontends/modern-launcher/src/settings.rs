use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::models::ReleaseMarket;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    #[default]
    Row,
    Column,
    Ribbon,
}

impl ViewMode {
    pub const ALL: [Self; 3] = [Self::Row, Self::Column, Self::Ribbon];

    pub const fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn is_vertical(self) -> bool {
        matches!(self, Self::Column)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Low,
    #[default]
    Medium,
    High,
}

impl Quality {
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];

    pub const fn artwork_size(self) -> u32 {
        match self {
            Self::Low => 512,
            Self::Medium => 1024,
            Self::High => 2048,
        }
    }

    pub const fn resident_neighbors(self) -> usize {
        match self {
            Self::Low => 5,
            Self::Medium => 8,
            Self::High => 10,
        }
    }

    pub const fn texture_budget_bytes(self) -> usize {
        match self {
            Self::Low => 48 * 1024 * 1024,
            Self::Medium => 128 * 1024 * 1024,
            Self::High => 256 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Binding {
    Up,
    Down,
    Left,
    Right,
    Previous,
    Next,
    PreviousLetter,
    NextLetter,
    PreviousConsole,
    NextConsole,
    Consoles,
    Search,
    Launch,
    Back,
    Flip,
    CycleView,
    Pause,
    QuickSave,
    QuickLoad,
    Settings,
    Fullscreen,
    Disabled,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GameButton {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    L,
    R,
    Start,
    Select,
    Disabled,
}

impl GameButton {
    pub const ALL: [Self; 12] = [
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
        Self::A,
        Self::B,
        Self::X,
        Self::Y,
        Self::L,
        Self::R,
        Self::Start,
        Self::Select,
    ];

    pub fn button(self) -> Option<core_model::Button> {
        use core_model::Button as B;

        Some(match self {
            Self::Up => B::Up,
            Self::Down => B::Down,
            Self::Left => B::Left,
            Self::Right => B::Right,
            Self::A => B::A,
            Self::B => B::B,
            Self::X => B::X,
            Self::Y => B::Y,
            Self::L => B::L,
            Self::R => B::R,
            Self::Start => B::Start,
            Self::Select => B::Select,
            Self::Disabled => return None,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Controls {
    pub repeat_delay: f64,
    pub repeat_interval: f64,
    pub stick_deadzone: f32,
    /// Winit physical key names, e.g. "KeyF". Entries override the defaults.
    pub keyboard: BTreeMap<String, Binding>,
    /// Gilrs button names, e.g. "South". Entries override the defaults.
    pub gamepad: BTreeMap<String, Binding>,
    pub game_keyboard: BTreeMap<String, GameButton>,
    pub game_gamepad: BTreeMap<String, GameButton>,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            repeat_delay: 0.35,
            repeat_interval: 0.085,
            stick_deadzone: 0.3,
            keyboard: BTreeMap::new(),
            gamepad: BTreeMap::new(),
            game_keyboard: BTreeMap::new(),
            game_gamepad: BTreeMap::new(),
        }
    }
}

impl Controls {
    pub const DEFAULT_KEYS: [(&str, Binding); 25] = [
        ("ArrowUp", Binding::Up),
        ("KeyW", Binding::Up),
        ("ArrowDown", Binding::Down),
        ("KeyS", Binding::Down),
        ("ArrowLeft", Binding::Left),
        ("KeyA", Binding::Left),
        ("ArrowRight", Binding::Right),
        ("KeyD", Binding::Right),
        ("Enter", Binding::Launch),
        ("Space", Binding::Launch),
        ("Escape", Binding::Back),
        ("KeyF", Binding::Flip),
        ("Tab", Binding::CycleView),
        ("KeyP", Binding::Pause),
        ("Pause", Binding::Pause),
        ("F5", Binding::QuickSave),
        ("F8", Binding::QuickLoad),
        ("F2", Binding::Settings),
        ("F11", Binding::Fullscreen),
        ("KeyQ", Binding::PreviousLetter),
        ("KeyE", Binding::NextLetter),
        ("PageUp", Binding::PreviousConsole),
        ("PageDown", Binding::NextConsole),
        ("KeyC", Binding::Consoles),
        ("Slash", Binding::Search),
    ];

    pub const DEFAULT_PAD: [(&str, Binding); 17] = [
        ("DPadUp", Binding::Up),
        ("DPadDown", Binding::Down),
        ("DPadLeft", Binding::Left),
        ("DPadRight", Binding::Right),
        ("South", Binding::Launch),
        ("East", Binding::Back),
        ("West", Binding::Flip),
        ("North", Binding::CycleView),
        ("Start", Binding::Consoles),
        ("Select", Binding::Settings),
        ("Mode", Binding::Pause),
        ("LeftThumb", Binding::Search),
        ("RightThumb", Binding::Settings),
        ("LeftTrigger", Binding::PreviousLetter),
        ("RightTrigger", Binding::NextLetter),
        ("LeftTrigger2", Binding::PreviousConsole),
        ("RightTrigger2", Binding::NextConsole),
    ];

    const GAME_KEYS: [(&str, GameButton); 12] = [
        ("ArrowUp", GameButton::Up),
        ("ArrowDown", GameButton::Down),
        ("ArrowLeft", GameButton::Left),
        ("ArrowRight", GameButton::Right),
        ("KeyZ", GameButton::B),
        ("KeyX", GameButton::A),
        ("KeyA", GameButton::Y),
        ("KeyS", GameButton::X),
        ("KeyQ", GameButton::L),
        ("KeyW", GameButton::R),
        ("Enter", GameButton::Start),
        ("Backspace", GameButton::Select),
    ];

    // Positional, so every controller matches the SNES pad layout.
    const GAME_PAD: [(&str, GameButton); 12] = [
        ("DPadUp", GameButton::Up),
        ("DPadDown", GameButton::Down),
        ("DPadLeft", GameButton::Left),
        ("DPadRight", GameButton::Right),
        ("South", GameButton::B),
        ("East", GameButton::A),
        ("West", GameButton::Y),
        ("North", GameButton::X),
        ("LeftTrigger", GameButton::L),
        ("RightTrigger", GameButton::R),
        ("Start", GameButton::Start),
        ("Select", GameButton::Select),
    ];

    pub fn keyboard_binding(&self, key: &str) -> Option<Binding> {
        resolve(&self.keyboard, &Self::DEFAULT_KEYS, key)
    }

    pub fn gamepad_binding(&self, button: &str) -> Option<Binding> {
        resolve(&self.gamepad, &Self::DEFAULT_PAD, button)
    }

    pub fn game_key(&self, key: &str) -> Option<core_model::Button> {
        resolve(&self.game_keyboard, &Self::GAME_KEYS, key).and_then(GameButton::button)
    }

    pub fn game_pad(&self, button: &str) -> Option<core_model::Button> {
        resolve(&self.game_gamepad, &Self::GAME_PAD, button).and_then(GameButton::button)
    }

    /// A remapped source is listed only under its new button.
    pub fn game_bindings(&self, button: GameButton) -> (Vec<String>, Vec<String>) {
        let collect = |defaults: &[(&str, GameButton)],
                       overrides: &BTreeMap<String, GameButton>,
                       reserved: &str| {
            let mut effective: BTreeMap<&str, GameButton> = defaults.iter().copied().collect();
            effective.extend(overrides.iter().map(|(key, value)| (key.as_str(), *value)));
            effective
                .into_iter()
                .filter(|(key, value)| *value == button && *key != reserved)
                .map(|(key, _)| key.to_owned())
                .collect()
        };

        (
            collect(&Self::GAME_KEYS, &self.game_keyboard, "Escape"),
            collect(&Self::GAME_PAD, &self.game_gamepad, "Mode"),
        )
    }
}

fn resolve<T: Copy>(
    overrides: &BTreeMap<String, T>,
    defaults: &[(&str, T)],
    name: &str,
) -> Option<T> {
    overrides.get(name).copied().or_else(|| {
        defaults
            .iter()
            .find_map(|(candidate, value)| (*candidate == name).then_some(*value))
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub rom_directories: Vec<PathBuf>,
    pub data_directory: Option<PathBuf>,
    pub layout: ViewMode,
    pub quality: Quality,
    pub reduced_motion: bool,
    pub text_scale: f32,
    pub integer_scaling: bool,
    pub fullscreen: bool,
    pub volume: f32,
    /// ROM SHA-1 to display market.
    pub release_overrides: BTreeMap<String, ReleaseMarket>,
    /// `system/sha1` to cartridge finish preset.
    pub cartridge_finishes: BTreeMap<String, String>,
    pub controls: Controls,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rom_directories: Vec::new(),
            data_directory: None,
            layout: ViewMode::Row,
            quality: Quality::Medium,
            reduced_motion: false,
            text_scale: 1.0,
            integer_scaling: true,
            fullscreen: false,
            volume: 0.8,
            release_overrides: BTreeMap::new(),
            cartridge_finishes: BTreeMap::new(),
            controls: Controls::default(),
        }
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("settings I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid settings: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("cannot encode settings: {0}")]
    Encode(#[from] toml::ser::Error),
    #[error("invalid setting: {0}")]
    Invalid(&'static str),
}

impl Settings {
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        Ok(Self::read(path)?.unwrap_or_default())
    }

    /// Falls back to the old `3d-launcher.toml` when the new file doesn't exist.
    pub fn load_default(path: &Path) -> Result<Self, SettingsError> {
        match Self::read(path)? {
            Some(settings) => Ok(settings),
            None => Self::load(&path.with_file_name("3d-launcher.toml")),
        }
    }

    fn read(path: &Path) -> Result<Option<Self>, SettingsError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(source) => {
                return Err(SettingsError::Io {
                    path: path.into(),
                    source,
                });
            }
        };

        Self::parse(&text).map(Some)
    }

    pub fn parse(text: &str) -> Result<Self, SettingsError> {
        let mut settings: Self = toml::from_str(text)?;

        for (hash, release) in std::mem::take(&mut settings.release_overrides) {
            if settings
                .release_overrides
                .insert(hash.to_ascii_lowercase(), release)
                .is_some()
            {
                return Err(SettingsError::Invalid(
                    "duplicate canonical ROM SHA-1 in release overrides",
                ));
            }
        }

        for (game, finish) in std::mem::take(&mut settings.cartridge_finishes) {
            if settings
                .cartridge_finishes
                .insert(game.to_ascii_lowercase(), finish)
                .is_some()
            {
                return Err(SettingsError::Invalid(
                    "duplicate console/ROM identity in cartridge finishes",
                ));
            }
        }

        settings.validate()?;
        Ok(settings)
    }

    pub fn validate(&self) -> Result<(), SettingsError> {
        if !self.text_scale.is_finite() || !(0.75..=2.0).contains(&self.text_scale) {
            return Err(SettingsError::Invalid(
                "text_scale must be between 0.75 and 2",
            ));
        }

        if !self.volume.is_finite() || !(0.0..=1.0).contains(&self.volume) {
            return Err(SettingsError::Invalid("volume must be between 0 and 1"));
        }

        let controls = &self.controls;

        if !controls.repeat_delay.is_finite()
            || !(0.1..=2.0).contains(&controls.repeat_delay)
            || !controls.repeat_interval.is_finite()
            || !(0.025..=1.0).contains(&controls.repeat_interval)
        {
            return Err(SettingsError::Invalid(
                "navigation repeat timing is outside its usable range",
            ));
        }

        if !controls.stick_deadzone.is_finite() || !(0.1..=0.8).contains(&controls.stick_deadzone) {
            return Err(SettingsError::Invalid(
                "stick_deadzone must be between 0.1 and 0.8",
            ));
        }

        if self.release_overrides.keys().any(|hash| !is_sha1(hash)) {
            return Err(SettingsError::Invalid(
                "release overrides require a 40-digit SHA-1 and a supported display market",
            ));
        }

        if self.cartridge_finishes.iter().any(|(key, finish)| {
            parse_game_setting_key(key).is_none_or(|game| {
                crate::materials::find_cartridge_finish(game.system, finish).is_none()
            })
        }) {
            return Err(SettingsError::Invalid(
                "cartridge finishes require a console/40-digit SHA-1 identity \
                 and a supported material preset",
            ));
        }

        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        self.validate()?;
        let text = toml::to_string_pretty(self)?;

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|source| SettingsError::Io {
                path: parent.into(),
                source,
            })?;
        }

        let temporary = path.with_extension(format!("toml.{}.tmp", std::process::id()));

        let result = (|| {
            use std::io::Write;
            let mut file = std::fs::File::create(&temporary)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            std::fs::rename(&temporary, path)
        })();

        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }

        result.map_err(|source| SettingsError::Io {
            path: path.into(),
            source,
        })
    }

    pub fn default_path() -> PathBuf {
        storage::FsStore::new().root().join("modern-launcher.toml")
    }

    pub fn cartridge_finish(&self, game: &core_model::GameId) -> Option<&str> {
        self.cartridge_finishes
            .get(&game_setting_key(game))
            .map(String::as_str)
    }

    pub fn set_cartridge_finish(&mut self, game: &core_model::GameId, finish: Option<&str>) {
        let key = game_setting_key(game);

        if let Some(finish) = finish {
            self.cartridge_finishes.insert(key, finish.into());
        } else {
            self.cartridge_finishes.remove(&key);
        }
    }
}

fn game_setting_key(game: &core_model::GameId) -> String {
    format!("{}/{}", game.system.slug(), game.sha1.to_ascii_lowercase())
}

fn parse_game_setting_key(key: &str) -> Option<core_model::GameId> {
    let (console, sha1) = key.split_once('/')?;

    let system = core_model::System::ALL
        .into_iter()
        .find(|system| system.slug() == console)?;

    is_sha1(sha1).then(|| core_model::GameId::new(system, sha1.to_ascii_lowercase()))
}

fn is_sha1(text: &str) -> bool {
    text.len() == 40 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}
