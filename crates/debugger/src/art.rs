use gpui::SharedString;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum System {
    #[allow(dead_code)]
    Nes,
    Snes,
}

#[derive(Clone)]
pub struct GameRef {
    pub system: System,
    pub name: SharedString,
}

impl GameRef {
    pub fn new(system: System, name: impl Into<SharedString>) -> Self {
        Self {
            system,
            name: name.into(),
        }
    }
}

pub trait AssetProvider: Send + Sync {
    fn cover_art(&self, game: &GameRef) -> Option<SharedString>;
}

pub struct NintendoGamesIcons {
    base_url: SharedString,
}

impl NintendoGamesIcons {
    pub fn new() -> Self {
        Self {
            base_url: "https://raw.githubusercontent.com/ZeroSuf3r/nintendo-games-icons/master"
                .into(),
        }
    }
}

impl Default for NintendoGamesIcons {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetProvider for NintendoGamesIcons {
    fn cover_art(&self, game: &GameRef) -> Option<SharedString> {
        if !matches!(game.system, System::Nes | System::Snes) {
            return None;
        }

        let name = game.name.trim();
        if name.is_empty() {
            return None;
        }

        let bucket = first_char_bucket(name);
        let file = urlencoding::encode(name);
        Some(format!("{}/{}/{}.png", self.base_url, bucket, file).into())
    }
}

fn first_char_bucket(name: &str) -> String {
    match name.chars().next() {
        Some(c) if c.is_ascii_digit() => "0-9".to_string(),
        Some(c) => c.to_ascii_uppercase().to_string(),
        None => "0-9".to_string(),
    }
}
