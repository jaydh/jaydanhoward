//! Real production model, ported near-verbatim from
//! `src/routes/security_audit.rs`: a separate periodic scanner (a small
//! standalone image running `cargo-audit` against this repo's lockfiles —
//! see the real `security-audit/entrypoint.py` and CI milestone 7's
//! `security-audit-image-push` job) POSTs the JSON report here with the
//! same Basic-Auth token Lighthouse uses. This process never runs
//! cargo-audit itself; it only stores and displays the latest report (see
//! `cluster.rs::fetch_security_audit` for the display side).

use axum::extract::State;
use axum::http::{header::HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use sqlx::PgPool;

const MAX_BODY_SIZE: usize = 1024 * 1024;

#[derive(thiserror::Error, Debug)]
enum SecurityAuditError {
    #[error("invalid credentials.")]
    InvalidCredentials,
    #[error("feature is disabled")]
    Disabled,
}

fn basic_authentication(headers: &HeaderMap) -> Result<(), SecurityAuditError> {
    let header_value = headers.get("authorization").ok_or(SecurityAuditError::InvalidCredentials)?;
    let header_str = header_value.to_str().map_err(|_| SecurityAuditError::InvalidCredentials)?;
    let encoded = header_str.strip_prefix("Basic ").ok_or(SecurityAuditError::InvalidCredentials)?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| SecurityAuditError::InvalidCredentials)?;
    let decoded = String::from_utf8(decoded).map_err(|_| SecurityAuditError::InvalidCredentials)?;

    let mut parts = decoded.splitn(2, ':');
    let username = parts.next().ok_or(SecurityAuditError::InvalidCredentials)?;
    let password = parts.next().ok_or(SecurityAuditError::InvalidCredentials)?;

    match std::env::var("LIGHTHOUSE_UPDATE_TOKEN") {
        Ok(val) => {
            if username != "jay" || password != val {
                return Err(SecurityAuditError::InvalidCredentials);
            }
        }
        Err(_) => return Err(SecurityAuditError::Disabled),
    }
    Ok(())
}

pub async fn upload_security_audit(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    match basic_authentication(&headers) {
        Ok(()) => {}
        Err(SecurityAuditError::Disabled) => return StatusCode::FORBIDDEN.into_response(),
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                [(
                    axum::http::header::WWW_AUTHENTICATE,
                    axum::http::header::HeaderValue::from_static("Basic realm=\"security-audit\""),
                )],
            )
                .into_response();
        }
    }

    if body.len() > MAX_BODY_SIZE {
        return (StatusCode::PAYLOAD_TOO_LARGE, "Exceeds 1MB limit").into_response();
    }

    let report: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid JSON").into_response(),
    };

    match sqlx::query("INSERT INTO security_audit (report, uploaded_at) VALUES ($1, NOW())")
        .bind(&report)
        .execute(&pool)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
