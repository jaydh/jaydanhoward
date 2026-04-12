#[cfg(feature = "ssr")]
use axum::http::StatusCode;

#[cfg(feature = "ssr")]
pub async fn health_check() -> StatusCode {
    StatusCode::OK
}
