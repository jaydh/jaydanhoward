#[cfg(feature = "ssr")]
use {
    axum::{
        extract::{Extension, Json},
        http::StatusCode,
        response::IntoResponse,
    },
    serde::Deserialize,
    sqlx::PgPool,
    std::sync::Arc,
    tracing::warn,
};

#[cfg(feature = "ssr")]
#[derive(Deserialize)]
pub struct ClaudeAuditPayload {
    pub context: String,
    pub model: String,
    pub prompt: String,
    pub response: Option<String>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub error: Option<String>,
}

#[cfg(feature = "ssr")]
pub async fn ingest_claude_audit(
    Extension(pool): Extension<Option<Arc<PgPool>>>,
    Json(payload): Json<ClaudeAuditPayload>,
) -> impl IntoResponse {
    let Some(pool) = pool else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    match crate::db::insert_claude_audit(
        &pool,
        &payload.context,
        &payload.model,
        &payload.prompt,
        payload.response.as_deref(),
        payload.input_tokens,
        payload.output_tokens,
        payload.error.as_deref(),
    )
    .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => {
            warn!("Failed to insert Claude audit from external service: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
