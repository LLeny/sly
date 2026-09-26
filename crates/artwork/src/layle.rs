use core_model::{GameId, Region};
use serde::Deserialize;

// Not security relevant, only used for metrics
const API_KEY: &str = "rad_layle_lIztEZYqfy_FCjjH6I6b99WaZ9i8-jWG1Sq-zlHHKME";

pub const BASE_URL: &str = "https://art.layle.dev";

pub fn authorization() -> String {
    format!("Bearer {API_KEY}")
}

pub fn is_art_url(url: &str) -> bool {
    url.starts_with(BASE_URL)
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub title: String,
    pub art: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ArtworkManifest {
    pub title: String,
    pub front: Option<String>,
    pub back: Option<String>,
    pub spine: Option<String>,
    pub label: Option<String>,
}

fn game_path(game: &GameId) -> String {
    format!("{}/{}", game.system.slug(), game.sha1)
}

pub fn metadata_url(game: &GameId) -> String {
    format!("{BASE_URL}/{}", game_path(game))
}

pub fn art_url(game: &GameId, art: &str) -> String {
    format!("{}?art={art}", metadata_url(game))
}

pub fn parse_metadata(bytes: &[u8]) -> Result<Metadata, serde_json::Error> {
    serde_json::from_slice(bytes)
}

/// `regions` is a priority list, most preferred first; an empty list falls back to any region.
pub fn resolve_metadata(
    game: &GameId,
    regions: &[Region],
    bytes: &[u8],
) -> Result<ArtworkManifest, serde_json::Error> {
    let metadata = parse_metadata(bytes)?;
    let url = |kind| select_art(&metadata.art, kind, regions).map(|art| art_url(game, &art));
    Ok(ArtworkManifest {
        title: metadata.title,
        front: url("box-front"),
        back: url("box-back"),
        spine: url("box-spine"),
        label: url("support-texture"),
    })
}

/// Picks the first art of `kind` matching `regions` in order, then any region, then any variant.
pub fn select_art(available: &[String], kind: &str, regions: &[Region]) -> Option<String> {
    const GLOBAL_FALLBACKS: &[&str] =
        &["wor", "us", "eu", "jp", "de", "fr", "sp", "au", "br", "ss"];

    let prefix = format!("{kind}/");
    regions
        .iter()
        .flat_map(|region| art_variants(*region))
        .chain(GLOBAL_FALLBACKS)
        .find_map(|variant| {
            let candidate = format!("{prefix}{variant}");
            let numbered = format!("{candidate}_");
            available
                .iter()
                .find(|item| **item == candidate || item.starts_with(&numbered))
                .cloned()
        })
        .or_else(|| {
            available
                .iter()
                .find(|item| item.starts_with(&prefix))
                .cloned()
        })
}

fn art_variants(region: Region) -> &'static [&'static str] {
    match region {
        Region::Japan | Region::China => &["jp"],
        Region::NorthAmerica | Region::SouthKorea => &["us"],
        Region::Canada => &["us", "fr"],
        Region::Brazil => &["br"],
        Region::Common | Region::Other(_) => &["wor"],
        Region::Germany => &["de", "eu"],
        Region::France => &["fr", "eu"],
        Region::Spain => &["sp", "eu"],
        Region::Australia => &["au", "eu"],
        Region::Scandinavia | Region::Finland | Region::Denmark => &["ss", "eu"],
        Region::Europe | Region::Netherlands | Region::Italy | Region::Indonesia => &["eu"],
    }
}

#[cfg(all(feature = "fetch", not(target_family = "wasm")))]
pub use fetch::{ArtworkBundle, fetch_bundle, fetch_wheel};

#[cfg(all(feature = "fetch", not(target_family = "wasm")))]
mod fetch {
    use std::path::PathBuf;
    use std::sync::LazyLock;
    use std::time::Duration;

    use core_model::{GameId, Region};

    use super::Metadata;

    static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
        ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(5)))
            .timeout_global(Some(Duration::from_secs(15)))
            .build()
            .new_agent()
    });

    #[derive(Debug)]
    pub struct ArtworkBundle {
        pub title: String,
        pub front: Option<Vec<u8>>,
        pub back: Option<Vec<u8>>,
        pub spine: Option<Vec<u8>>,
        pub label: Option<Vec<u8>>,
    }

    pub fn fetch_bundle(game: &GameId, region: Option<Region>) -> Result<ArtworkBundle, String> {
        if let Some(cached) = load_cached_bundle(game) {
            return Ok(cached);
        }

        let metadata = fetch_metadata(game)?;

        let bundle = ArtworkBundle {
            title: metadata.title,
            front: fetch_kind(game, &metadata.art, "box-front", region),
            back: fetch_kind(game, &metadata.art, "box-back", region),
            spine: fetch_kind(game, &metadata.art, "box-spine", region),
            label: fetch_kind(game, &metadata.art, "support-texture", region),
        };

        if bundle.front.is_some() {
            write_cached_bundle(game, &bundle);
        }

        Ok(bundle)
    }

    pub fn fetch_wheel(game: &GameId, region: Option<Region>) -> Result<Option<Vec<u8>>, String> {
        let cache_name = format!(
            "wheel-{}",
            region
                .map(super::art_variants)
                .unwrap_or_default()
                .join("-")
        );

        let path = bundle_cache_dir(game).join(&cache_name);
        if let Some(cached) = read_cache_bytes(&path) {
            return Ok(Some(cached));
        }

        let metadata = fetch_metadata(game)?;
        let wheel = fetch_kind(game, &metadata.art, "wheel", region);
        if let Some(bytes) = &wheel {
            write_cache_bytes(game, &cache_name, bytes);
        }

        Ok(wheel)
    }

    fn cache_root() -> PathBuf {
        dirs::cache_dir()
            .map(|dir| dir.join("sly").join("artwork"))
            .unwrap_or_else(|| PathBuf::from("artwork"))
    }

    fn bundle_cache_dir(game: &GameId) -> PathBuf {
        cache_root().join(super::game_path(game))
    }

    fn load_cached_bundle(game: &GameId) -> Option<ArtworkBundle> {
        let dir = bundle_cache_dir(game);
        let front = read_cache_bytes(&dir.join("front"))?;
        let title = std::fs::read_to_string(dir.join("title"))
            .unwrap_or_default()
            .trim()
            .to_string();
        Some(ArtworkBundle {
            title,
            front: Some(front),
            back: read_cache_bytes(&dir.join("back")),
            spine: read_cache_bytes(&dir.join("spine")),
            label: read_cache_bytes(&dir.join("label")),
        })
    }

    fn read_cache_bytes(path: &std::path::Path) -> Option<Vec<u8>> {
        std::fs::read(path).ok().filter(|bytes| !bytes.is_empty())
    }

    fn fetch_metadata(game: &GameId) -> Result<Metadata, String> {
        get(&super::metadata_url(game))
            .map_err(|err| format!("metadata unavailable: {err}"))?
            .body_mut()
            .read_json()
            .map_err(|err| format!("invalid artwork metadata: {err}"))
    }

    fn write_cache_bytes(game: &GameId, name: &str, bytes: &[u8]) {
        let dir = bundle_cache_dir(game);
        if let Err(err) = std::fs::create_dir_all(&dir) {
            eprintln!("artwork cache: failed to create {}: {err}", dir.display());
            return;
        }

        if let Err(err) = std::fs::write(dir.join(name), bytes) {
            eprintln!("artwork cache: failed to write {name} for {game:?}: {err}");
        }
    }

    fn write_cached_bundle(game: &GameId, bundle: &ArtworkBundle) {
        let dir = bundle_cache_dir(game);
        if let Err(err) = std::fs::create_dir_all(&dir) {
            eprintln!("artwork cache: failed to create {}: {err}", dir.display());
            return;
        }

        let write = |name: &str, bytes: &Option<Vec<u8>>| {
            if let Some(bytes) = bytes
                && let Err(err) = std::fs::write(dir.join(name), bytes)
            {
                eprintln!("artwork cache: failed to write {name} for {game:?}: {err}");
            }
        };

        write("front", &bundle.front);
        write("back", &bundle.back);
        write("spine", &bundle.spine);
        write("label", &bundle.label);

        if let Err(err) = std::fs::write(dir.join("title"), &bundle.title) {
            eprintln!("artwork cache: failed to write title for {game:?}: {err}");
        }
    }

    fn get(url: &str) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
        AGENT
            .get(url)
            .header("Authorization", super::authorization())
            .call()
    }

    fn fetch_kind(
        game: &GameId,
        available: &[String],
        kind: &str,
        region: Option<Region>,
    ) -> Option<Vec<u8>> {
        let selected = super::select_art(available, kind, region.as_slice())?;
        get(&super::art_url(game, &selected))
            .and_then(|response| response.into_body().read_to_vec())
            .ok()
    }
}
