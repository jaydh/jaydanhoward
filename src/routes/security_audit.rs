#[cfg(feature = "ssr")]
#[derive(thiserror::Error, Debug)]
pub enum SecurityAuditError {
    #[error("invalid credentials.")]
    InvalidCredentials(),
    #[error("Feature is disabled")]
    DisabledError(),
}

#[cfg(feature = "ssr")]
const MAX_FILE_SIZE: usize = 1024 * 1024; // 1MB

#[cfg(feature = "ssr")]
use {
    axum::{
        body::Bytes,
        http::{header::HeaderMap, StatusCode},
        response::{IntoResponse, Response},
    },
    base64::Engine,
    tracing::{instrument, log},
};

#[cfg(feature = "ssr")]
fn basic_authentication(headers: &HeaderMap) -> Result<(), SecurityAuditError> {
    let header_value = headers
        .get("authorization")
        .ok_or(SecurityAuditError::InvalidCredentials())?;

    let header_str = header_value
        .to_str()
        .map_err(|_| SecurityAuditError::InvalidCredentials())?;

    let base64encoded_credentials = header_str
        .strip_prefix("Basic ")
        .ok_or(SecurityAuditError::InvalidCredentials())?;

    let decoded_credentials = base64::engine::general_purpose::STANDARD
        .decode(base64encoded_credentials)
        .map_err(|_| SecurityAuditError::InvalidCredentials())?;

    let decoded_credentials = String::from_utf8(decoded_credentials)
        .map_err(|_| SecurityAuditError::InvalidCredentials())?;

    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or(SecurityAuditError::InvalidCredentials())?
        .to_string();
    let password = credentials
        .next()
        .ok_or(SecurityAuditError::InvalidCredentials())?
        .to_string();

    match std::env::var("LIGHTHOUSE_UPDATE_TOKEN") {
        Ok(val) => {
            if username != "jay" || password != val {
                return Err(SecurityAuditError::InvalidCredentials());
            }
        }
        Err(_) => return Err(SecurityAuditError::DisabledError()),
    }

    Ok(())
}

#[cfg(feature = "ssr")]
#[instrument(skip(body))]
pub async fn upload_security_audit(headers: HeaderMap, body: Bytes) -> Response {
    log::info!("Received upload_security_audit");

    match basic_authentication(&headers) {
        Ok(()) => {}
        Err(SecurityAuditError::DisabledError()) => {
            return StatusCode::FORBIDDEN.into_response();
        }
        Err(e) => {
            let ip = headers
                .get("x-real-ip")
                .or_else(|| headers.get("x-forwarded-for"))
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown");
            log::warn!("Security audit auth failure from {ip}: {e}");
            return (
                StatusCode::UNAUTHORIZED,
                [(
                    axum::http::header::WWW_AUTHENTICATE,
                    axum::http::header::HeaderValue::from_static(
                        "Basic realm=\"security-audit\"",
                    ),
                )],
            )
                .into_response();
        }
    }

    if body.len() > MAX_FILE_SIZE {
        log::warn!("Upload rejected: body size {} exceeds limit", body.len());
        return (StatusCode::PAYLOAD_TOO_LARGE, "Exceeds 1MB limit").into_response();
    }

    // Validate it's parseable JSON before writing
    if serde_json::from_slice::<serde_json::Value>(&body).is_err() {
        log::warn!("Upload rejected: body is not valid JSON");
        return (StatusCode::BAD_REQUEST, "Invalid JSON").into_response();
    }

    use runfiles::{rlocation, Runfiles};
    let r = Runfiles::create().expect("Must run using bazel with runfiles");
    let assets_path = rlocation!(r, "_main/assets").expect("Failed to locate assets");
    let file_path = format!("{}/security-audit.json", assets_path.to_string_lossy());

    match std::fs::write(&file_path, &body) {
        Ok(()) => {
            log::info!("Written security audit to {file_path}");
            StatusCode::OK.into_response()
        }
        Err(e) => {
            log::error!("Failed to write security audit: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
