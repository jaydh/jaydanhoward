//! Cross-cutting HTTP middleware ported from the real site's
//! `src/middleware/{cache_control,security_headers,rate_limit}.rs`. All
//! three are framework-agnostic axum middleware with no Leptos dependency,
//! so this is a verbatim port apart from one deliberate adaptation noted
//! on `cache_control` below.

use axum::extract::Request;
use axum::http::header::{
    HeaderName, HeaderValue, CACHE_CONTROL, CONTENT_SECURITY_POLICY, REFERRER_POLICY,
    STRICT_TRANSPORT_SECURITY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::middleware::Next;
use axum::response::Response;
use axum::{http::StatusCode, response::IntoResponse};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Short content hash of `files` (missing files hash as empty), for `?v=`
/// cache-busting. Same binary + same files ⇒ same value on every replica.
pub fn content_version(files: &[String]) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for f in files {
        f.hash(&mut h);
        std::fs::read(f).unwrap_or_default().hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

/// Every file under `dir`, recursively, sorted (for [`content_version`]).
pub fn files_under(dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![std::path::PathBuf::from(dir)];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p.to_string_lossy().into_owned());
            }
        }
    }
    out.sort();
    out
}

/// (`/pkg` version, `/widgets` version) this replica serves, set at startup.
static ASSET_VERSIONS: OnceLock<(String, String)> = OnceLock::new();

pub fn set_asset_versions(pkg: String, widgets: String) {
    let _ = ASSET_VERSIONS.set((pkg, widgets));
}

/// Cache policy for the WASM bundles under `/pkg` and `/widgets`.
/// - `?v=<this replica's version>`: immutable — the URL changes on every
///   deploy, since the page (never cached) stamps the current version in.
/// - `?v=<anything else>`: `no-store`. Mid-rollout, a new page's request can
///   land on an old replica; caching its old bytes under the new URL would
///   pin a mismatched pair at the edge for a year.
/// - no `v` (the glue's own relative loads, e.g. a widget's .wasm or a
///   wasm-bindgen snippet): `no-cache`, i.e. always revalidate.
fn bundle_cache_policy(path: &str, query: Option<&str>) -> Option<&'static str> {
    let expected = match ASSET_VERSIONS.get() {
        Some((pkg, _)) if path.starts_with("/pkg/") => pkg,
        Some((_, widgets)) if path.starts_with("/widgets/") => widgets,
        _ if path.starts_with("/pkg/") || path.starts_with("/widgets/") => return Some("no-cache"),
        _ => return None,
    };
    let v = query.and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("v=")));
    Some(match v {
        Some(v) if v == expected => "public, max-age=31536000, immutable",
        Some(_) => "no-store",
        None => "no-cache",
    })
}

fn has_hash_segment(path: &str) -> bool {
    path.split('/').any(|seg| seg.len() >= 8 && seg.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Same extension-based policy as the real site, plus versioned caching for
/// the WASM bundles (`/pkg`, `/widgets`) — see [`bundle_cache_policy`].
pub async fn cache_control(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(str::to_string);
    let mut response = next.run(req).await;

    if response.headers().contains_key(CACHE_CONTROL) {
        return response;
    }

    let cache_header = if let Some(policy) = bundle_cache_policy(&path, query.as_deref()) {
        policy
    } else if path.ends_with(".js") && has_hash_segment(&path) {
        "public, max-age=31536000, immutable"
    } else if path.ends_with(".js") {
        "public, max-age=3600"
    } else if path.ends_with(".woff2")
        || path.ends_with(".woff")
        || path.ends_with(".ttf")
        || path.ends_with(".eot")
        || path.ends_with(".otf")
    {
        "public, max-age=31536000, immutable"
    } else if path.ends_with(".webp")
        || path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".gif")
        || path.ends_with(".svg")
        || path.ends_with(".ico")
        || path.ends_with(".css")
    {
        "public, max-age=2592000, must-revalidate"
    } else if path.ends_with(".html") || path == "/" {
        "public, max-age=0, must-revalidate"
    } else {
        "public, max-age=86400"
    };

    response.headers_mut().insert(CACHE_CONTROL, HeaderValue::from_static(cache_header));
    response
}

pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();

    headers.insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; \
             script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval' https://static.cloudflareinsights.com; \
             style-src 'self' 'unsafe-inline'; \
             img-src 'self' https://caddy.jaydanhoward.com data:; \
             media-src 'self' https://caddy.jaydanhoward.com; \
             font-src 'self'; \
             connect-src 'self' https://cloudflareinsights.com; \
             frame-src 'self'; \
             frame-ancestors 'self'; \
             base-uri 'self'; \
             form-action 'self';",
        ),
    );
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("SAMEORIGIN"));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(STRICT_TRANSPORT_SECURITY, HeaderValue::from_static("max-age=31536000; includeSubDomains"));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("strict-origin-when-cross-origin"));
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(
            "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
        ),
    );
    headers.insert(HeaderName::from_static("x-xss-protection"), HeaderValue::from_static("1; mode=block"));
    headers.insert(HeaderName::from_static("cross-origin-opener-policy"), HeaderValue::from_static("same-origin"));
    headers.insert(HeaderName::from_static("cross-origin-resource-policy"), HeaderValue::from_static("same-origin"));

    response
}

#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<RateLimitState>>,
    max_requests: usize,
    window: Duration,
}

struct RateLimitState {
    clients: HashMap<String, (usize, Instant)>,
    last_cleanup: Instant,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(RateLimitState { clients: HashMap::new(), last_cleanup: Instant::now() })),
            max_requests,
            window,
        }
    }

    fn check_rate_limit(&self, ip: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();

        if now.duration_since(state.last_cleanup) > Duration::from_secs(300) {
            state.clients.retain(|_, (_, start)| now.duration_since(*start) < self.window);
            state.last_cleanup = now;
        }

        let entry = state.clients.entry(ip.to_string()).or_insert((0, now));

        if now.duration_since(entry.1) >= self.window {
            entry.0 = 1;
            entry.1 = now;
            return true;
        }

        if entry.0 >= self.max_requests {
            return false;
        }

        entry.0 += 1;
        true
    }

    pub async fn check_middleware(&self, req: Request, next: Next) -> Response {
        let ip = req
            .headers()
            .get("x-real-ip")
            .or_else(|| req.headers().get("x-forwarded-for"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        if !self.check_rate_limit(&ip) {
            return (StatusCode::TOO_MANY_REQUESTS, "Too many requests. Please try again later.").into_response();
        }

        next.run(req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_policy() {
        set_asset_versions("p1".into(), "w1".into());
        assert_eq!(bundle_cache_policy("/pkg/foster_client.js", Some("v=p1")), Some("public, max-age=31536000, immutable"));
        assert_eq!(bundle_cache_policy("/pkg/foster_client.js", Some("v=old")), Some("no-store"));
        assert_eq!(bundle_cache_policy("/pkg/snippets/x/inline0.js", None), Some("no-cache"));
        assert_eq!(bundle_cache_policy("/widgets/life/life_widget.js", Some("v=w1")), Some("public, max-age=31536000, immutable"));
        assert_eq!(bundle_cache_policy("/widgets/life/life_widget_bg.wasm", None), Some("no-cache"));
        assert_eq!(bundle_cache_policy("/favicon.ico", Some("v=p1")), None);
    }
}
