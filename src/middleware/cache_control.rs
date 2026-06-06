use axum::{extract::Request, middleware::Next, response::Response};
use axum::http::header::{HeaderValue, CACHE_CONTROL};

fn has_hash_segment(path: &str) -> bool {
    path.split('/').any(|seg| seg.len() >= 8 && seg.chars().all(|c| c.is_ascii_hexdigit()))
}

pub async fn cache_control(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let mut response = next.run(req).await;

    let cache_header = if path.ends_with(".wasm") {
        "public, max-age=3600"
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
        "public, max-age=604800, must-revalidate"
    } else if path.ends_with(".html") || path == "/" {
        "public, max-age=0, must-revalidate"
    } else {
        "public, max-age=86400"
    };

    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static(cache_header),
    );

    response
}
