use gpui::SharedString;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum System {
    #[allow(dead_code)]
    Nes,
    Snes,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Header {
    None,
    SnesCopier,
}

#[derive(Clone, Copy)]
pub struct SystemInfo {
    pub libretro_name: &'static str,
    pub header: Header,
}

impl System {
    pub const fn info(self) -> SystemInfo {
        match self {
            System::Nes => SystemInfo {
                libretro_name: "Nintendo - Nintendo Entertainment System",
                header: Header::None,
            },
            System::Snes => SystemInfo {
                libretro_name: "Nintendo - Super Nintendo Entertainment System",
                header: Header::SnesCopier,
            },
        }
    }
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

pub struct LibretroThumbnails {
    base_url: SharedString,
}

impl LibretroThumbnails {
    pub fn new() -> Self {
        Self {
            base_url: "https://thumbnails.libretro.com".into(),
        }
    }
}

impl Default for LibretroThumbnails {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetProvider for LibretroThumbnails {
    fn cover_art(&self, game: &GameRef) -> Option<SharedString> {
        let name = game.name.trim();
        if name.is_empty() {
            return None;
        }

        let system = urlencoding::encode(game.system.info().libretro_name);
        let sanitized = sanitize(name);
        let file = urlencoding::encode(&sanitized);
        Some(format!("{}/{}/Named_Boxarts/{}.png", self.base_url, system, file).into())
    }
}

fn sanitize(name: &str) -> String {
    const SPECIAL: &str = "&*/:`<>?\\|\"";
    name.chars()
        .map(|c| if SPECIAL.contains(c) { '_' } else { c })
        .collect()
}
