use std::sync::LazyLock;
use std::time::Duration;

use core_model::Region;
use serde::Deserialize;

// Not security relevant, only used for metrics
const API_KEY: &str = "rad_layle_lIztEZYqfy_FCjjH6I6b99WaZ9i8-jWG1Sq-zlHHKME";

const BASE_URL: &str = "https://art.layle.dev";

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

#[derive(Debug, Deserialize)]
struct Metadata {
    title: String,
    art: Vec<String>,
}

pub fn fetch_bundle(sha1: &str, region: Region) -> Result<ArtworkBundle, String> {
    let metadata: Metadata = get(sha1)
        .map_err(|err| format!("metadata unavailable: {err}"))?
        .body_mut()
        .read_json()
        .map_err(|err| format!("invalid artwork metadata: {err}"))?;

    let variants = preferred_art_variants(region);
    Ok(ArtworkBundle {
        title: metadata.title,
        front: fetch_kind(sha1, &metadata.art, "box-front", variants),
        back: fetch_kind(sha1, &metadata.art, "box-back", variants),
        spine: fetch_kind(sha1, &metadata.art, "box-spine", variants),
        label: fetch_kind(sha1, &metadata.art, "support-texture", variants),
    })
}

fn get(path_and_query: &str) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
    AGENT
        .get(format!("{BASE_URL}/snes/{path_and_query}"))
        .header("Authorization", format!("Bearer {API_KEY}"))
        .call()
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

fn select_art(available: &[String], kind: &str, preferred_variants: &[&str]) -> Option<String> {
    let prefix = format!("{kind}/");
    preferred_variants
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

fn fetch_kind(
    sha1: &str,
    available: &[String],
    kind: &str,
    preferred_variants: &[&str],
) -> Option<Vec<u8>> {
    let selected = select_art(available, kind, preferred_variants)?;
    get(&format!("{sha1}?art={selected}"))
        .and_then(|response| response.into_body().read_to_vec())
        .ok()
}
