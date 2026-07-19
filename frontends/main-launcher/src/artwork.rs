use sha1::{Digest, Sha1};

// Not security relevant, only used for metrics
const API_KEY: &str = "rad_layle_lIztEZYqfy_FCjjH6I6b99WaZ9i8-jWG1Sq-zlHHKME";

pub fn fetch_label(console: &str, rom: &str) -> Vec<u8> {
    let buffer = std::fs::read(rom).expect("Failed to read ROM file");
    let sha1 = Sha1::digest(&buffer)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<String>();
    let url = format!(
        "https://art.layle.dev/{}/{}?art=support-texture/eu",
        console, sha1
    );

    let client = reqwest::blocking::Client::new();
    let response = client
        .get(&url)
        .bearer_auth(API_KEY)
        .send()
        .expect("Failed to fetch label")
        .error_for_status()
        .expect("Label not available");
    response
        .bytes()
        .expect("Failed to read label bytes")
        .to_vec()
}
