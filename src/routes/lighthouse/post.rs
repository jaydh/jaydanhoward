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
use {
    actix_multipart::Multipart,
    actix_web::http::header::HeaderMap,
    actix_web::{Error, HttpRequest, HttpResponse},
    base64::Engine,
    futures_util::StreamExt as _,
    std::io::Write,
    tracing::{instrument, log},
};

#[cfg(feature = "ssr")]
fn basic_authentication(headers: &HeaderMap) -> Result<(), LighthouseError> {
    // The header value, if present, must be a valid UTF8 string
    let header_value = headers.get("Authorization").unwrap().to_str().unwrap();
    let base64encoded_credentials = header_value.strip_prefix("Basic ").unwrap();
    let decoded_credentials = base64::engine::general_purpose::STANDARD
        .decode(base64encoded_credentials)
        .unwrap();
    let decoded_credentials = String::from_utf8(decoded_credentials).unwrap();

    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials.next().unwrap().to_string();
    let password = credentials.next().unwrap().to_string();

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
#[instrument(skip(payload))]
pub async fn upload_lighthouse_report(
    request: HttpRequest,
    mut payload: Multipart,
) -> Result<HttpResponse, Error> {
    log::info!("Recieved upload_lighthouse_report");
    let credentials = basic_authentication(request.headers());
    if credentials.is_err() {
        return Ok(HttpResponse::BadRequest().finish());
    }

    log::info!("Valid credentials upload_lighthouse_report");

    let r = Runfiles::create().expect("Must run using bazel with runfiles");
    let assets_path = rlocation!(r, "_main/assets").expect("Failed to locate main");
    let file_path = format!("{}/lighthouse.html", assets_path.to_string_lossy());
    let mut file = std::fs::OpenOptions::new()
        .truncate(true)
        .write(true)
        .open(&file_path)?;

    let mut file_contents: Vec<u8> = Vec::new();
    while let Some(item) = payload.next().await {
        let mut field = item?;
        while let Some(chunk) = field.next().await {
            let chunk = chunk?;
            file_contents.extend_from_slice(&chunk);
        }
    }
    let _ = file.write_all(&file_contents);

    log::info!("Successfully written report to {}", &file_path);

    Ok(HttpResponse::Ok().finish())
}
