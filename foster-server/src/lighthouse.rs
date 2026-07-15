//! Real production model, ported verbatim from
//! `src/routes/lighthouse/post.rs`: an external CI job (see
//! `lighthouse/entrypoint.sh` in the real repo) POSTs a static Lighthouse
//! HTML report to this Basic-Auth-protected endpoint once per deploy. The
//! site does not audit itself live — that was a PoC-only adaptation for a
//! demo with no CI pipeline of its own.

use axum::extract::Multipart;
use axum::http::{header::HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use base64::Engine;

const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

#[derive(thiserror::Error, Debug)]
enum LighthouseError {
    #[error("invalid credentials.")]
    InvalidCredentials,
    #[error("feature is disabled")]
    Disabled,
}

fn basic_authentication(headers: &HeaderMap) -> Result<(), LighthouseError> {
    let header_value = headers
        .get("authorization")
        .ok_or(LighthouseError::InvalidCredentials)?;
    let header_str = header_value.to_str().map_err(|_| LighthouseError::InvalidCredentials)?;
    let encoded = header_str
        .strip_prefix("Basic ")
        .ok_or(LighthouseError::InvalidCredentials)?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| LighthouseError::InvalidCredentials)?;
    let decoded = String::from_utf8(decoded).map_err(|_| LighthouseError::InvalidCredentials)?;

    let mut parts = decoded.splitn(2, ':');
    let username = parts.next().ok_or(LighthouseError::InvalidCredentials)?;
    let password = parts.next().ok_or(LighthouseError::InvalidCredentials)?;

    match std::env::var("LIGHTHOUSE_UPDATE_TOKEN") {
        Ok(val) => {
            if username != "jay" || password != val {
                return Err(LighthouseError::InvalidCredentials);
            }
        }
        Err(_) => return Err(LighthouseError::Disabled),
    }

    Ok(())
}

pub async fn upload_lighthouse_report(headers: HeaderMap, mut multipart: Multipart) -> Response {
    match basic_authentication(&headers) {
        Ok(()) => {}
        Err(LighthouseError::Disabled) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                [(
                    axum::http::header::WWW_AUTHENTICATE,
                    axum::http::header::HeaderValue::from_static("Basic realm=\"lighthouse\""),
                )],
            )
                .into_response();
        }
    }

    // Same runtime-path fallback as main.rs's static_dir: the compile-time
    // CARGO_MANIFEST_DIR is the Docker builder stage's path, which doesn't
    // exist in the final image — only /app/static does.
    let static_dir = if std::path::Path::new("/app/static").exists() {
        "/app/static".to_string()
    } else {
        concat!(env!("CARGO_MANIFEST_DIR"), "/static").to_string()
    };
    let file_path = format!("{static_dir}/lighthouse.html");

    let mut file = match std::fs::OpenOptions::new().create(true).truncate(true).write(true).open(&file_path) {
        Ok(f) => f,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let mut file_contents: Vec<u8> = Vec::new();
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
        let data = match field.bytes().await {
            Ok(d) => d,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
        if file_contents.len() + data.len() > MAX_FILE_SIZE {
            return (StatusCode::PAYLOAD_TOO_LARGE, "File size exceeds maximum allowed size of 10MB")
                .into_response();
        }
        file_contents.extend_from_slice(&data);
    }

    use std::io::Write;
    let _ = file.write_all(&file_contents);
    StatusCode::OK.into_response()
}
