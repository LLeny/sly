use crate::System;

const BASE_URL: &str = "https://raw.githubusercontent.com/libretro-thumbnails";

pub fn cover_url(system: System, name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }

    let sanitized = self::sanitize(name);
    let file = urlencoding::encode(&sanitized);
    let repo = system.info().libretro_name.replace(' ', "_");
    Some(format!("{BASE_URL}/{repo}/master/Named_Boxarts/{file}.png"))
}

fn sanitize(name: &str) -> String {
    const SPECIAL: &str = "&*/:`<>?\\|\"";
    name.chars()
        .map(|c| if SPECIAL.contains(c) { '_' } else { c })
        .collect()
}
