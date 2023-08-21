#[cfg(feature = "ssr")]
use {
    actix_multipart::Multipart,
    actix_web::http::header::{HeaderMap, HeaderValue},
    actix_web::{Error, HttpRequest, HttpResponse},
    base64::Engine,
    futures_util::StreamExt as _,
    secrecy::{ExposeSecret, Secret},
    std::io::Write,
};

#[cfg(feature = "ssr")]
struct Credentials {
    username: String,
    password: Secret<String>,
}
#[cfg(feature = "ssr")]
fn basic_authentication(headers: &HeaderMap) -> Result<(), Error> {
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

    if username != "jay" || password != "ab" {
        return Err(Error::try_from("blah"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
pub async fn upload_lighthouse_report(
    request: HttpRequest,
    mut payload: Multipart,
) -> Result<HttpResponse, Error> {
    let credentials = basic_authentication(request.headers());

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open("site/lighthouse.html")?;

    let mut file_contents: Vec<u8> = Vec::new();
    while let Some(item) = payload.next().await {
        let mut field = item?;
        while let Some(chunk) = field.next().await {
            let chunk = chunk?;
            file_contents.extend_from_slice(&chunk);
        }
    }
    let _ = file.write_all(&file_contents);

    Ok(HttpResponse::Ok().into())
}
