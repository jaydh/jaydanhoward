//! Real photo gallery — points straight at the real, live production CDN
//! (`caddy.jaydanhoward.com`, a Cloudflare-tunneled Caddy file server on the
//! author's actual homelab, confirmed publicly reachable right now) rather
//! than standing up an equivalent. Same JSON directory-listing + filename
//! convention as the real site's `photography.rs`.

use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct FileItem {
    name: String,
    is_dir: bool,
}

fn fetch_blocking() -> Result<Vec<Value>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let body = client
        .get("https://caddy.jaydanhoward.com")
        .header("Accept", "application/json")
        .send()
        .map_err(|e| e.to_string())?
        .text()
        .map_err(|e| e.to_string())?;

    let files: Vec<FileItem> = serde_json::from_str(&body).map_err(|e| e.to_string())?;

    let mut photos = Vec::new();
    for file in files {
        if file.is_dir {
            continue;
        }
        let name_lower = file.name.to_lowercase();
        let Some(base) = name_lower
            .ends_with("-full.webp")
            .then(|| file.name.trim_end_matches("-full.webp").trim_end_matches("-full.WEBP"))
        else {
            continue;
        };
        if base.contains("..") || base.starts_with('/') {
            continue;
        }
        let index = photos.len();
        photos.push(json!({
            "index": index,
            "name": base,
            "thumb_url": format!("https://caddy.jaydanhoward.com/{base}-thumb.webp"),
            "medium_url": format!("https://caddy.jaydanhoward.com/{base}-medium.webp"),
            "full_url": format!("https://caddy.jaydanhoward.com/{base}-full.webp"),
        }));
    }
    Ok(photos)
}

/// Fetches the real photo listing once. Blocking (network I/O), called via
/// `block_in_place` at startup, same pattern as cluster.rs/visitors.rs.
pub fn fetch_photos() -> Value {
    let photos = tokio::task::block_in_place(fetch_blocking).unwrap_or_default();
    json!({ "photos": photos, "viewing_index": -1 })
}
