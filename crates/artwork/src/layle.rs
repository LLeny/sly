use core_model::Region;
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

pub fn metadata_url(sha1: &str) -> String {
    format!("{BASE_URL}/snes/{sha1}")
}

pub fn art_url(sha1: &str, art: &str) -> String {
    format!("{BASE_URL}/snes/{sha1}?art={art}")
}

pub fn parse_metadata(bytes: &[u8]) -> Result<Metadata, serde_json::Error> {
    serde_json::from_slice(bytes)
}

pub fn select_art(available: &[String], kind: &str, region: Region) -> Option<String> {
    let prefix = format!("{kind}/");
    preferred_art_variants(region)
        .iter()
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

fn preferred_art_variants(region: Region) -> &'static [&'static str] {
    match region {
        Region::Japan => &["jp", "us", "wor", "eu"],
        Region::NorthAmerica | Region::SouthKorea => &["us", "jp", "wor", "eu"],
        Region::Canada => &["us", "fr", "jp", "wor", "eu"],
        Region::Brazil => &["br", "us", "jp", "wor", "eu"],
        Region::China => &["jp", "us", "wor", "eu"],
        Region::Common | Region::Other(_) => &["wor", "us", "jp", "eu"],
        Region::Germany => &["de", "eu", "fr", "sp", "au", "wor", "us"],
        Region::France => &["fr", "eu", "de", "sp", "au", "wor", "us"],
        Region::Spain => &["sp", "eu", "fr", "de", "au", "wor", "us"],
        Region::Australia => &["au", "eu", "de", "fr", "sp", "wor", "us"],
        Region::Scandinavia | Region::Finland | Region::Denmark => {
            &["ss", "eu", "de", "fr", "sp", "au", "wor", "us"]
        }
        Region::Europe | Region::Netherlands | Region::Italy | Region::Indonesia => {
            &["eu", "de", "fr", "sp", "au", "wor", "us"]
        }
    }
}

#[cfg(all(feature = "fetch", not(target_family = "wasm")))]
pub use fetch::{ArtworkBundle, fetch_bundle};

#[cfg(all(feature = "fetch", not(target_family = "wasm")))]
mod fetch {
    use std::path::PathBuf;
    use std::sync::LazyLock;
    use std::time::Duration;

    use core_model::Region;

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

    pub fn fetch_bundle(sha1: &str, region: Region) -> Result<ArtworkBundle, String> {
        if let Some(cached) = load_cached_bundle(sha1) {
            return Ok(cached);
        }

        let metadata: Metadata = get(&super::metadata_url(sha1))
            .map_err(|err| format!("metadata unavailable: {err}"))?
            .body_mut()
            .read_json()
            .map_err(|err| format!("invalid artwork metadata: {err}"))?;

        let bundle = ArtworkBundle {
            title: metadata.title,
            front: fetch_kind(sha1, &metadata.art, "box-front", region),
            back: fetch_kind(sha1, &metadata.art, "box-back", region),
            spine: fetch_kind(sha1, &metadata.art, "box-spine", region),
            label: fetch_kind(sha1, &metadata.art, "support-texture", region),
        };

        if bundle.front.is_some() {
            write_cached_bundle(sha1, &bundle);
        }
        Ok(bundle)
    }

    fn cache_root() -> PathBuf {
        dirs::cache_dir()
            .map(|dir| dir.join("sly").join("artwork"))
            .unwrap_or_else(|| PathBuf::from("artwork"))
    }

    fn bundle_cache_dir(sha1: &str) -> PathBuf {
        cache_root().join(sha1)
    }

    fn load_cached_bundle(sha1: &str) -> Option<ArtworkBundle> {
        let dir = bundle_cache_dir(sha1);
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

    fn write_cached_bundle(sha1: &str, bundle: &ArtworkBundle) {
        let dir = bundle_cache_dir(sha1);
        if let Err(err) = std::fs::create_dir_all(&dir) {
            eprintln!("artwork cache: failed to create {}: {err}", dir.display());
            return;
        }

        let write = |name: &str, bytes: &Option<Vec<u8>>| {
            if let Some(bytes) = bytes
                && let Err(err) = std::fs::write(dir.join(name), bytes)
            {
                eprintln!("artwork cache: failed to write {name} for {sha1}: {err}");
            }
        };

        write("front", &bundle.front);
        write("back", &bundle.back);
        write("spine", &bundle.spine);
        write("label", &bundle.label);

        if let Err(err) = std::fs::write(dir.join("title"), &bundle.title) {
            eprintln!("artwork cache: failed to write title for {sha1}: {err}");
        }
    }

    fn get(url: &str) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
        AGENT
            .get(url)
            .header("Authorization", super::authorization())
            .call()
    }

    fn fetch_kind(sha1: &str, available: &[String], kind: &str, region: Region) -> Option<Vec<u8>> {
        let selected = super::select_art(available, kind, region)?;
        get(&super::art_url(sha1, &selected))
            .and_then(|response| response.into_body().read_to_vec())
            .ok()
    }
}
