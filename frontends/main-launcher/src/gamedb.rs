use std::collections::HashMap;
use std::sync::OnceLock;

const NO_INTRO_DAT: &str = include_str!(
    "../../../submodules/libretro-database/metadat/no-intro/Nintendo - Super Nintendo Entertainment System.dat"
);
const GENRE_DAT: &str = include_str!(
    "../../../submodules/libretro-database/metadat/genre/Nintendo - Super Nintendo Entertainment System.dat"
);
const DEVELOPER_DAT: &str = include_str!(
    "../../../submodules/libretro-database/metadat/developer/Nintendo - Super Nintendo Entertainment System.dat"
);
const PUBLISHER_DAT: &str = include_str!(
    "../../../submodules/libretro-database/metadat/publisher/Nintendo - Super Nintendo Entertainment System.dat"
);
const RELEASE_YEAR_DAT: &str = include_str!(
    "../../../submodules/libretro-database/metadat/releaseyear/Nintendo - Super Nintendo Entertainment System.dat"
);
const RELEASE_MONTH_DAT: &str = include_str!(
    "../../../submodules/libretro-database/metadat/releasemonth/Nintendo - Super Nintendo Entertainment System.dat"
);
const MAX_USERS_DAT: &str = include_str!(
    "../../../submodules/libretro-database/metadat/maxusers/Nintendo - Super Nintendo Entertainment System.dat"
);

#[derive(Clone, Debug, Default)]
pub struct GameMeta {
    pub name: Option<String>,
    pub genre: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub release_year: Option<u16>,
    pub release_month: Option<u8>,
    pub max_users: Option<u8>,
}

impl GameMeta {
    pub fn display_title(&self) -> Option<String> {
        let name = self.name.as_deref()?;
        let trimmed = name.split(" (").next().unwrap_or(name).trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }

    /// Non-allocating equivalent of `display_title().is_some()`.
    pub fn has_normalized_title(&self) -> bool {
        let Some(name) = self.name.as_deref() else {
            return false;
        };
        let trimmed = name.split(" (").next().unwrap_or(name).trim();
        !trimmed.is_empty()
    }

    pub fn released_label(&self) -> Option<String> {
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let year = self.release_year?;
        match self.release_month {
            Some(month) if (1..=12).contains(&month) => {
                Some(format!("{} {year}", MONTHS[month as usize - 1]))
            }
            _ => Some(year.to_string()),
        }
    }

    pub fn players_label(&self) -> Option<String> {
        match self.max_users? {
            0 => None,
            1 => Some("1".to_string()),
            n => Some(format!("1-{n}")),
        }
    }
}

#[derive(Default)]
struct FieldRecord {
    genre: Option<String>,
    developer: Option<String>,
    publisher: Option<String>,
    release_year: Option<u16>,
    release_month: Option<u8>,
    max_users: Option<u8>,
}

struct GameDb {
    names_by_sha1: HashMap<String, String>,
    names_by_crc: HashMap<u32, String>,
    fields_by_crc: HashMap<u32, FieldRecord>,
}

pub fn lookup(sha1: &str, crc: u32) -> GameMeta {
    let db = database();

    let mut meta = GameMeta::default();
    meta.name = db
        .names_by_sha1
        .get(&sha1.to_ascii_lowercase())
        .or_else(|| db.names_by_crc.get(&crc))
        .cloned();

    if let Some(fields) = db.fields_by_crc.get(&crc) {
        meta.genre = fields.genre.clone();
        meta.developer = fields.developer.clone();
        meta.publisher = fields.publisher.clone();
        meta.release_year = fields.release_year;
        meta.release_month = fields.release_month;
        meta.max_users = fields.max_users;
    }

    meta
}

fn database() -> &'static GameDb {
    static DB: OnceLock<GameDb> = OnceLock::new();

    DB.get_or_init(|| {
        let mut db = GameDb {
            names_by_sha1: HashMap::new(),
            names_by_crc: HashMap::new(),
            fields_by_crc: HashMap::new(),
        };

        for_each_game(NO_INTRO_DAT, |fields, rom| {
            let Some(name) = fields.get("name") else {
                return;
            };
            if let Some(sha1) = rom.get("sha1") {
                db.names_by_sha1
                    .insert(sha1.to_ascii_lowercase(), name.clone());
            }
            if let Some(crc) = parse_crc(rom) {
                db.names_by_crc.entry(crc).or_insert_with(|| name.clone());
            }
        });

        let mut merge = |source: &str, apply: fn(&mut FieldRecord, &HashMap<String, String>)| {
            for_each_game(source, |fields, rom| {
                let Some(crc) = parse_crc(rom) else {
                    return;
                };
                apply(db.fields_by_crc.entry(crc).or_default(), fields);
            });
        };
        merge(GENRE_DAT, |record, fields| {
            record.genre = fields.get("genre").cloned();
        });
        merge(DEVELOPER_DAT, |record, fields| {
            record.developer = fields.get("developer").cloned();
        });
        merge(PUBLISHER_DAT, |record, fields| {
            record.publisher = fields.get("publisher").cloned();
        });
        merge(RELEASE_YEAR_DAT, |record, fields| {
            record.release_year = fields
                .get("releaseyear")
                .and_then(|value| value.parse().ok());
        });
        merge(RELEASE_MONTH_DAT, |record, fields| {
            record.release_month = fields
                .get("releasemonth")
                .and_then(|value| value.parse().ok());
        });
        merge(MAX_USERS_DAT, |record, fields| {
            record.max_users = fields.get("users").and_then(|value| value.parse().ok());
        });

        db
    })
}

fn parse_crc(rom: &HashMap<String, String>) -> Option<u32> {
    u32::from_str_radix(rom.get("crc")?, 16).ok()
}

fn for_each_game(
    source: &str,
    mut visit: impl FnMut(&HashMap<String, String>, &HashMap<String, String>),
) {
    let mut in_game = false;
    let mut fields: HashMap<String, String> = HashMap::new();
    let mut rom: HashMap<String, String> = HashMap::new();

    for line in source.lines() {
        let line = line.trim();
        if !in_game {
            if line.starts_with("game (") {
                in_game = true;
                fields.clear();
                rom.clear();
            }
            continue;
        }

        if line == ")" {
            in_game = false;
            visit(&fields, &rom);
        } else if let Some(rest) = line.strip_prefix("rom (") {
            if rom.is_empty() {
                let tokens = tokenize(rest.trim_end_matches(')'));
                for pair in tokens.chunks_exact(2) {
                    rom.insert(pair[0].clone(), pair[1].clone());
                }
            }
        } else if let Some((key, value)) = line.split_once(char::is_whitespace) {
            fields.insert(key.to_string(), value.trim().trim_matches('"').to_string());
        }
    }
}

fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for character in line.chars() {
        match character {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}
