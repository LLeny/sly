pub(super) struct Sources {
    pub names: &'static str,
    pub genre: &'static str,
    pub developer: &'static str,
    pub publisher: &'static str,
    pub release_year: &'static str,
    pub release_month: &'static str,
    pub max_users: &'static str,
}

macro_rules! catalog {
    ($system:literal) => {
        Sources {
            names: include_str!(concat!(
                "../../../submodules/libretro-database/metadat/no-intro/",
                $system,
                ".dat"
            )),
            genre: include_str!(concat!(
                "../../../submodules/libretro-database/metadat/genre/",
                $system,
                ".dat"
            )),
            developer: include_str!(concat!(
                "../../../submodules/libretro-database/metadat/developer/",
                $system,
                ".dat"
            )),
            publisher: include_str!(concat!(
                "../../../submodules/libretro-database/metadat/publisher/",
                $system,
                ".dat"
            )),
            release_year: include_str!(concat!(
                "../../../submodules/libretro-database/metadat/releaseyear/",
                $system,
                ".dat"
            )),
            release_month: include_str!(concat!(
                "../../../submodules/libretro-database/metadat/releasemonth/",
                $system,
                ".dat"
            )),
            max_users: include_str!(concat!(
                "../../../submodules/libretro-database/metadat/maxusers/",
                $system,
                ".dat"
            )),
        }
    };
}

pub(super) const SNES: Sources = catalog!("Nintendo - Super Nintendo Entertainment System");
pub(super) const GBA: Sources = catalog!("Nintendo - Game Boy Advance");
pub(super) const LYNX: Sources = catalog!("Atari - Lynx");
