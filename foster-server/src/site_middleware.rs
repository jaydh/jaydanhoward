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
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn has_hash_segment(path: &str) -> bool {
    path.split('/').any(|seg| seg.len() >= 8 && seg.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Same extension-based policy as the real site, with one adaptation: the
/// real `/jaydanhoward_wasm/?v={hash}` cache-busting scheme relies on
/// Leptos's build embedding a content hash in the query string. Foster's
/// `/pkg` (wasm-pack output) has no such versioning yet, so treating it as
/// immutable would leave browsers pinned to a stale WASM/JS pair after a
/// deploy. Until `/pkg` gets a real cache-busting mechanism, it deliberately
/// falls through to the same short-lived default every uncategorized route
/// gets, rather than copying the immutable rule to something unsafe.
pub async fn cache_control(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let mut response = next.run(req).await;

    if response.headers().contains_key(CACHE_CONTROL) {
        return response;
    }

    let cache_header = if path.ends_with(".js") && has_hash_segment(&path) {
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
