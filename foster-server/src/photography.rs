//! Photo gallery backed by the homelab's `files-public` server. The gallery
//! isn't public on its own — files.jaydanhoward.com only routes share links
//! (`/s/<id>`) — so this server reads the listing and proxies the renditions
//! at `/photos/<file>` from the in-cluster Service. In-cluster also sidesteps
//! the pod's gluetun sidecar, which blocks non-cluster egress until the VPN is
//! up (exactly when the startup listing fetch runs).

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Overridable for local dev (e.g. a `kubectl port-forward` to files-public).
fn photo_origin() -> String {
    std::env::var("PHOTO_ORIGIN")
        .unwrap_or_else(|_| "http://files-public.media.svc.cluster.local:8000".to_string())
}

#[derive(Deserialize)]
struct FileItem {
    name: String,
    is_dir: bool,
}

fn fetch_blocking(origin: &str) -> Result<Vec<Value>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let body = client
        .get(origin)
        .header("Accept", "application/json")
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
        .map_err(|e| format!("{origin}: {e}"))?;

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
        if !is_safe_name(base) {
            continue;
        }
        let index = photos.len();
        photos.push(json!({
            "index": index,
            "name": base,
            "thumb_url": format!("/photos/{base}-thumb.webp"),
            "medium_url": format!("/photos/{base}-medium.webp"),
            "full_url": format!("/photos/{base}-full.webp"),
        }));
    }
    Ok(photos)
}

/// Fetches the photo listing once. Blocking (network I/O), called via
/// `block_in_place` at startup, same pattern as cluster.rs/visitors.rs. The
/// list is baked into the page, so a failure here means an empty gallery
/// until the next restart — retry a few times and log rather than silently
/// shipping `[]`.
pub fn fetch_photos() -> Value {
    const ATTEMPTS: u32 = 5;
    let origin = photo_origin();
    let photos = tokio::task::block_in_place(|| {
        for attempt in 1..=ATTEMPTS {
            match fetch_blocking(&origin) {
                Ok(photos) => return photos,
                Err(e) => {
                    eprintln!("photography: listing fetch {attempt}/{ATTEMPTS} failed: {e}");
                    if attempt < ATTEMPTS {
                        std::thread::sleep(std::time::Duration::from_secs(2 * attempt as u64));
                    }
                }
            }
        }
        Vec::new()
    });
    json!({ "photos": photos })
}

fn is_safe_name(name: &str) -> bool {
    !name.is_empty() && !name.contains("..") && !name.contains('/') && !name.contains('\\')
}

#[derive(Clone)]
struct ProxyState {
    client: reqwest::Client,
    origin: String,
}

/// `/photos/<file>`: streams a `.webp` rendition from files-public. Only flat
/// `.webp` names — the listing and anything else on that server stay private.
async fn photo(State(state): State<ProxyState>, Path(file): Path<String>) -> Response {
    if !is_safe_name(&file) || !file.to_lowercase().ends_with(".webp") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let upstream = match state.client.get(format!("{}/{file}", state.origin)).send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("photography: proxy {file}: {e}");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };
    if !upstream.status().is_success() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let mut builder = Response::builder().header(header::CONTENT_TYPE, "image/webp");
    for name in [header::CONTENT_LENGTH, header::ETAG, header::LAST_MODIFIED] {
        if let Some(v) = upstream.headers().get(&name) {
            builder = builder.header(name, v.clone());
        }
    }
    builder
        .body(Body::from_stream(upstream.bytes_stream()))
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}

pub fn router() -> Router {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("reqwest client");
    Router::new()
        .route("/photos/{file}", get(photo))
        .with_state(ProxyState { client, origin: photo_origin() })
}
