use core_model::{GameId, Region};
use serde::Deserialize;

pub const BASE_URL: &str = "https://art.layle.dev";

pub fn is_art_url(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("art.layle.dev")
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none()
    })
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

/// A market's own scans first, then markets that shared its packaging.
fn art_variants(region: Region) -> &'static [&'static str] {
    match region {
        Region::Japan => &["jp"],
        Region::China => &["cn", "jp"],
        Region::NorthAmerica => &["us"],
        Region::SouthKorea => &["kr", "us"],
        Region::Canada => &["ca", "us", "fr"],
        Region::Brazil => &["br"],
        Region::Common | Region::Other(_) => &["wor"],
        Region::Europe => &["eu"],
        Region::Germany => &["de", "eu"],
        Region::France => &["fr", "eu"],
        Region::Spain => &["sp", "eu"],
        Region::Italy => &["it", "eu"],
        Region::Netherlands => &["nl", "eu"],
        Region::Australia => &["au", "eu"],
        Region::Scandinavia => &["ss", "eu"],
        Region::Finland => &["fi", "ss", "eu"],
        Region::Denmark => &["dk", "ss", "eu"],
        Region::Indonesia => &["id", "eu"],
    }
}

/// Art for exactly this market. Unknown markets match worldwide art.
pub fn select_art_strict(
    available: &[String],
    kind: &str,
    region: Option<Region>,
) -> Option<String> {
    let variant = region.map_or("wor", |region| art_variants(region)[0]);

    let exact = format!("{kind}/{variant}");

    if available.contains(&exact) {
        return Some(exact);
    }

    let numbered = format!("{exact}_");

    available
        .iter()
        .filter(|name| {
            name.strip_prefix(&numbered).is_some_and(|suffix| {
                !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
            })
        })
        .min()
        .cloned()
}

#[cfg(all(feature = "fetch", not(target_family = "wasm")))]
pub use fetch::{ArtworkBundle, fetch_bundle_strict, fetch_wheel};

#[cfg(all(feature = "fetch", not(target_family = "wasm")))]
mod fetch {
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use std::sync::LazyLock;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use core_model::{GameId, Region};

    use super::Metadata;

    const MAX_METADATA_BYTES: u64 = 512 * 1024;
    const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
    const METADATA_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

    static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
        ureq::Agent::config_builder()
            .max_redirects(0)
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
        pub label_front: Option<Vec<u8>>,
        pub label_top: Option<Vec<u8>>,
    }

    pub fn fetch_bundle_strict(
        game: &GameId,
        region: Option<Region>,
        cancelled: impl Fn() -> bool,
    ) -> Result<ArtworkBundle, String> {
        let check = || {
            if cancelled() {
                Err("Artwork request cancelled".to_string())
            } else {
                Ok(())
            }
        };

        check()?;

        let directory = cache_directory(game);
        let metadata_path = directory.join("metadata-v1.json");

        let fresh = std::fs::metadata(&metadata_path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age < METADATA_MAX_AGE);

        let cached = read_bounded(&metadata_path, MAX_METADATA_BYTES);

        let (metadata, refreshed) = metadata_with_fallback(cached.as_deref(), fresh, || {
            get_bounded(&super::metadata_url(game), MAX_METADATA_BYTES)
        })?;

        if let Some(bytes) = refreshed {
            write_atomic(&metadata_path, &bytes);
        }

        let sources = directory.join("sources");

        let fetch = |kind: &str| -> Result<Option<Vec<u8>>, String> {
            check()?;

            let Some(selected) = super::select_art_strict(&metadata.art, kind, region) else {
                return Ok(None);
            };

            // `selected` is a known role and variant, so it is a safe path.
            let path = sources.join(&selected);

            if let Some(bytes) = read_bounded(&path, MAX_IMAGE_BYTES) {
                return Ok(Some(bytes));
            }

            match get_bounded(&super::art_url(game, &selected), MAX_IMAGE_BYTES) {
                Ok(bytes) => {
                    check()?;
                    write_atomic(&path, &bytes);
                    Ok(Some(bytes))
                }
                Err(error) => Err(format!("artwork {selected}: {error}")),
            }
        };

        let front = fetch("box-front")?;
        let back = fetch("box-back")?;
        let spine = fetch("box-spine")?;
        let label = fetch("support-texture")?;
        let label_front = fetch("support-texture-front")?;
        let label_top = fetch("support-texture-top")?;

        Ok(ArtworkBundle {
            title: metadata.title,
            front,
            back,
            spine,
            label,
            label_front,
            label_top,
        })
    }

    pub fn fetch_wheel(game: &GameId, region: Option<Region>) -> Result<Option<Vec<u8>>, String> {
        let variants = region.map(super::art_variants).unwrap_or_default();
        let path = cache_directory(game).join(format!("wheel-{}", variants.join("-")));

        if let Some(cached) = read_bounded(&path, MAX_IMAGE_BYTES) {
            return Ok(Some(cached));
        }

        let metadata = get_bounded(&super::metadata_url(game), MAX_METADATA_BYTES)
            .map_err(|error| format!("metadata unavailable: {error}"))?;
        let metadata = super::parse_metadata(&metadata)
            .map_err(|error| format!("invalid artwork metadata: {error}"))?;

        let Some(selected) = super::select_art(&metadata.art, "wheel", region.as_slice()) else {
            return Ok(None);
        };

        let wheel = Some(get_bounded(
            &super::art_url(game, &selected),
            MAX_IMAGE_BYTES,
        )?);

        if let Some(bytes) = &wheel {
            write_atomic(&path, bytes);
        }

        Ok(wheel)
    }

    /// Uses fresh cached metadata as is, and stale cache if the refresh fails.
    fn metadata_with_fallback(
        cached: Option<&[u8]>,
        fresh: bool,
        refresh: impl FnOnce() -> Result<Vec<u8>, String>,
    ) -> Result<(Metadata, Option<Vec<u8>>), String> {
        let cached = cached.and_then(|bytes| super::parse_metadata(bytes).ok());

        if fresh && let Some(metadata) = cached {
            return Ok((metadata, None));
        }

        let refreshed = refresh().and_then(|bytes| {
            let metadata = super::parse_metadata(&bytes).map_err(|error| error.to_string())?;
            Ok((metadata, Some(bytes)))
        });

        refreshed.or_else(|error| cached.map(|metadata| (metadata, None)).ok_or(error))
    }

    fn cache_directory(game: &GameId) -> PathBuf {
        dirs::cache_dir()
            .map(|dir| dir.join("sly").join("artwork"))
            .unwrap_or_else(|| PathBuf::from("artwork"))
            .join(super::game_path(game))
    }

    fn read_bounded(path: &Path, limit: u64) -> Option<Vec<u8>> {
        let file = std::fs::File::open(path).ok()?;

        if file.metadata().ok()?.len() > limit {
            return None;
        }

        let mut bytes = Vec::new();
        file.take(limit + 1).read_to_end(&mut bytes).ok()?;
        (!bytes.is_empty() && bytes.len() as u64 <= limit).then_some(bytes)
    }

    /// Write a temp file and rename it so readers never see a partial file.
    fn write_atomic(path: &Path, bytes: &[u8]) {
        static NEXT_WRITE: AtomicU64 = AtomicU64::new(0);

        let Some(parent) = path.parent() else {
            return;
        };

        if std::fs::create_dir_all(parent).is_err() {
            return;
        }

        let temporary = path.with_extension(format!(
            "{}-{}.tmp",
            std::process::id(),
            NEXT_WRITE.fetch_add(1, Ordering::Relaxed)
        ));

        if std::fs::write(&temporary, bytes).is_ok() {
            let _ = std::fs::rename(&temporary, path);
        }

        let _ = std::fs::remove_file(temporary);
    }

    fn get_bounded(url: &str, limit: u64) -> Result<Vec<u8>, String> {
        let mut request = AGENT.get(url);

        if super::is_art_url(url)
            && let Some(value) = settings::authorization()
        {
            request = request.header("Authorization", value);
        }

        request
            .call()
            .map_err(|error| match error {
                ureq::Error::StatusCode(401 | 403) => {
                    "Artwork API key rejected; update it in Settings".to_string()
                }
                ureq::Error::StatusCode(429) => {
                    "Artwork rate limit reached; try later or set a key in Settings".to_string()
                }
                error => error.to_string(),
            })?
            .body_mut()
            .with_config()
            .limit(limit)
            .read_to_vec()
            .map_err(|error| error.to_string())
    }
}
