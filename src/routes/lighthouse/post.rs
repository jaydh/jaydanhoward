use runfiles::{rlocation, Runfiles};

#[cfg(feature = "ssr")]
#[derive(thiserror::Error, Debug)]
pub enum LighthouseError {
    #[error("invalid credentials.")]
    InvalidCredentials(),
    #[error("Feature is disabled")]
    DisabledError(),
}

// Maximum file upload size: 10MB
#[cfg(feature = "ssr")]
const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

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
    // Get Authorization header
    let header_value = headers
        .get("Authorization")
        .ok_or(LighthouseError::InvalidCredentials())?;

    // Convert to string
    let header_str = header_value
        .to_str()
        .map_err(|_| LighthouseError::InvalidCredentials())?;

    // Strip "Basic " prefix
    let base64encoded_credentials = header_str
        .strip_prefix("Basic ")
        .ok_or(LighthouseError::InvalidCredentials())?;

    // Decode base64
    let decoded_credentials = base64::engine::general_purpose::STANDARD
        .decode(base64encoded_credentials)
        .map_err(|_| LighthouseError::InvalidCredentials())?;

    // Convert to UTF-8 string
    let decoded_credentials = String::from_utf8(decoded_credentials)
        .map_err(|_| LighthouseError::InvalidCredentials())?;

    // Split on ':' to get username and password
    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or(LighthouseError::InvalidCredentials())?
        .to_string();
    let password = credentials
        .next()
        .ok_or(LighthouseError::InvalidCredentials())?
        .to_string();

    // Validate credentials
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
        .create(true)
        .truncate(true)
        .write(true)
        .open(&file_path)
        .expect("Failed to open file");

    let mut file_contents: Vec<u8> = Vec::new();
    while let Some(item) = payload.next().await {
        let mut field = item?;
        while let Some(chunk) = field.next().await {
            let chunk = chunk?;

            // Check if adding this chunk would exceed the max file size
            if file_contents.len() + chunk.len() > MAX_FILE_SIZE {
                log::warn!("Upload rejected: file size exceeds {} bytes", MAX_FILE_SIZE);
                return Ok(HttpResponse::PayloadTooLarge()
                    .body("File size exceeds maximum allowed size of 10MB"));
            }

            file_contents.extend_from_slice(&chunk);
        }
    }

    let _ = file.write_all(&file_contents);

    log::info!("Successfully written report to {}", &file_path);

    Ok(HttpResponse::Ok().finish())
}
