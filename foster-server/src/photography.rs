//! Photo gallery. The webp renditions live on the homelab's photo volume,
//! mounted read-only at `PHOTO_DIR` (homelab `service/jaydanhoward-photos.yaml`)
//! and served at `/photos/<file>` — they aren't exposed anywhere else.

use axum::Router;
use serde_json::{json, Value};
use std::path::PathBuf;
use tower_http::services::ServeDir;

/// Unset (local dev, CI) means an empty gallery.
fn photo_dir() -> Option<PathBuf> {
    std::env::var_os("PHOTO_DIR").map(PathBuf::from)
}

/// Lists the photos once at startup; the list is baked into the page.
pub fn fetch_photos() -> Value {
    let mut bases = Vec::new();
    if let Some(dir) = photo_dir() {
        match std::fs::read_dir(&dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if !entry.file_type().is_ok_and(|t| t.is_file()) {
                        continue;
                    }
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if let Some(base) = name.strip_suffix("-full.webp") {
                        bases.push(base.to_string());
                    }
                }
            }
            Err(e) => eprintln!("photography: reading {}: {e}", dir.display()),
        }
    }
    bases.sort();

    let photos: Vec<Value> = bases
        .iter()
        .enumerate()
        .map(|(index, base)| {
            json!({
                "index": index,
                "name": base,
                "thumb_url": format!("/photos/{base}-thumb.webp"),
                "medium_url": format!("/photos/{base}-medium.webp"),
                "full_url": format!("/photos/{base}-full.webp"),
            })
        })
        .collect();
    json!({ "photos": photos })
}

pub fn router() -> Router {
    match photo_dir() {
        Some(dir) => Router::new().nest_service("/photos", ServeDir::new(dir)),
        None => Router::new(),
    }
}
