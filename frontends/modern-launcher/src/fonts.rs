use glyphon::{FontSystem, fontdb};

const SANS: &[u8] = include_bytes!("../../../vendor/fonts/ibm-plex/IBMPlexSans-Regular.ttf");
const SEMIBOLD: &[u8] = include_bytes!("../../../vendor/fonts/ibm-plex/IBMPlexSans-SemiBold.ttf");
const MONO: &[u8] = include_bytes!("../../../vendor/fonts/ibm-plex/IBMPlexMono-Regular.ttf");

pub fn configure() -> (FontSystem, Vec<String>) {
    let mut database = fontdb::Database::new();
    database.load_system_fonts();

    for font in [SANS, SEMIBOLD, MONO] {
        database.load_font_data(font.to_vec());
    }

    database.set_sans_serif_family("IBM Plex Sans");
    database.set_monospace_family("IBM Plex Mono");

    let families = [
        fontdb::Family::Name("Noto Sans CJK JP"),
        fontdb::Family::Name("Noto Sans JP"),
        fontdb::Family::Name("Yu Gothic"),
        fontdb::Family::Name("Yu Gothic UI"),
        fontdb::Family::Name("Hiragino Sans"),
        fontdb::Family::Name("Hiragino Kaku Gothic ProN"),
        fontdb::Family::Name("Meiryo"),
        fontdb::Family::Name("TakaoGothic"),
        fontdb::Family::Name("IPAexGothic"),
    ];

    let japanese = database.query(&fontdb::Query {
        families: &families,
        ..Default::default()
    });

    let warnings = if japanese.is_some() {
        Vec::new()
    } else {
        vec![
            "No preferred Japanese font found. Install Noto Sans CJK JP for Japanese titles."
                .into(),
        ]
    };

    // Prefer Japanese glyph forms for shared Han characters.
    (
        FontSystem::new_with_locale_and_db("ja-JP".into(), database),
        warnings,
    )
}
