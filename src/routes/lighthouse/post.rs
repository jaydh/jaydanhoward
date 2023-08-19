#[cfg(feature = "ssr")]
use actix_web::{web, HttpResponse};

#[derive(serde::Deserialize)]
pub struct FormData {}

#[cfg(feature = "ssr")]
pub async fn upload_lighthouse_report(
    form: web::Form<FormData>,
) -> Result<HttpResponse, actix_web::Error> {
    Ok(HttpResponse::Ok().finish())
}
