#[cfg(feature = "ssr")]
use {
    axum::{http::StatusCode, response::{IntoResponse, Response}},
    std::fs,
    tracing::instrument,
};

#[cfg(feature = "ssr")]
#[instrument]
pub async fn robots_txt() -> Response {
    match fs::read_to_string("assets/robots.txt") {
        Ok(content) => content.into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
