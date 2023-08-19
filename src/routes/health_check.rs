#[cfg(feature = "ssr")]
use actix_web::HttpResponse;

#[cfg(feature = "ssr")]
pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().finish()
}
