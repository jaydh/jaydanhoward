#[cfg(feature = "ssr")]
use {actix_web::HttpResponse, tracing::instrument};

#[cfg(feature = "ssr")]
#[instrument]
pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().finish()
}
