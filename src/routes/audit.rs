#[cfg(feature = "ssr")]
use {
    actix_web::{web, HttpResponse},
    serde::Deserialize,
    sqlx::PgPool,
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
    pool: web::Data<PgPool>,
    payload: web::Json<ClaudeAuditPayload>,
) -> HttpResponse {
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
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            warn!("Failed to insert Claude audit from external service: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}
