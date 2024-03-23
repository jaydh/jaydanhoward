#[cfg(feature = "ssr")]
use actix_web::{web, HttpRequest, Result};
use std::fs;

#[cfg(feature = "ssr")]
pub async fn robots_txt(_req: HttpRequest) -> Result<web::Bytes> {
    let content = fs::read_to_string("assets/robots.txt")
        .map_err(|_| actix_web::error::ErrorNotFound("robots.txt not found"))?;

    Ok(web::Bytes::from(content))
}
