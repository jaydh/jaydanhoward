use runfiles::{rlocation, Runfiles};

#[cfg(feature = "ssr")]
#[derive(thiserror::Error, Debug)]
pub enum LighthouseError {
    #[error("invalid credentials.")]
    InvalidCredentials(),
    #[error("Feature is disabled")]
    DisabledError(),
}

#[cfg(feature = "ssr")]
const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

#[cfg(feature = "ssr")]
use {
    axum::{
        extract::Multipart,
        http::{header::HeaderMap, StatusCode},
        response::{IntoResponse, Response},
    },
    base64::Engine,
    tracing::{instrument, log},
};

#[cfg(feature = "ssr")]
fn basic_authentication(headers: &HeaderMap) -> Result<(), LighthouseError> {
    let header_value = headers
        .get("authorization")
        .ok_or(LighthouseError::InvalidCredentials())?;

    let header_str = header_value
        .to_str()
        .map_err(|_| LighthouseError::InvalidCredentials())?;

    let base64encoded_credentials = header_str
        .strip_prefix("Basic ")
        .ok_or(LighthouseError::InvalidCredentials())?;

    let decoded_credentials = base64::engine::general_purpose::STANDARD
        .decode(base64encoded_credentials)
        .map_err(|_| LighthouseError::InvalidCredentials())?;

    let decoded_credentials = String::from_utf8(decoded_credentials)
        .map_err(|_| LighthouseError::InvalidCredentials())?;

    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or(LighthouseError::InvalidCredentials())?
        .to_string();
    let password = credentials
        .next()
        .ok_or(LighthouseError::InvalidCredentials())?
        .to_string();

    match std::env::var("LIGHTHOUSE_UPDATE_TOKEN") {
        Ok(val) => {
            if username != "jay" || password != val {
                return Err(LighthouseError::InvalidCredentials());
            }
        }
        Err(_) => return Err(LighthouseError::DisabledError()),
    }

    Ok(())
}

#[cfg(feature = "ssr")]
#[instrument(skip(multipart))]
pub async fn upload_lighthouse_report(
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    log::info!("Received upload_lighthouse_report");
    match basic_authentication(&headers) {
        Ok(()) => {}
        Err(LighthouseError::DisabledError()) => {
            return StatusCode::FORBIDDEN.into_response();
        }
        Err(e) => {
            let ip = headers
                .get("x-real-ip")
                .or_else(|| headers.get("x-forwarded-for"))
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown");
            log::warn!("Lighthouse auth failure from {ip}: {e}");
            return (
                StatusCode::UNAUTHORIZED,
                [(
                    axum::http::header::WWW_AUTHENTICATE,
                    axum::http::header::HeaderValue::from_static(
                        "Basic realm=\"lighthouse\"",
                    ),
                )],
            )
                .into_response();
        }
    }

    log::info!("Valid credentials upload_lighthouse_report");

    let r = Runfiles::create().expect("Must run using bazel with runfiles");
    let assets_path = rlocation!(r, "_main/assets").expect("Failed to locate main");
    let file_path = format!("{}/lighthouse.html", assets_path.to_string_lossy());

    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&file_path)
    {
        Ok(f) => f,
        Err(e) => {
            log::error!("Failed to open lighthouse file: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut file_contents: Vec<u8> = Vec::new();
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                log::warn!("Multipart error: {e}");
                return StatusCode::BAD_REQUEST.into_response();
            }
        };

        let data = match field.bytes().await {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Field bytes error: {e}");
                return StatusCode::BAD_REQUEST.into_response();
            }
        };

        if file_contents.len() + data.len() > MAX_FILE_SIZE {
            log::warn!("Upload rejected: file size exceeds {MAX_FILE_SIZE} bytes");
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                "File size exceeds maximum allowed size of 10MB",
            )
                .into_response();
        }

        file_contents.extend_from_slice(&data);
    }

    use std::io::Write;
    let _ = file.write_all(&file_contents);

    log::info!("Successfully written report to {}", &file_path);
    StatusCode::OK.into_response()
}
