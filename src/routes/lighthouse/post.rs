#[cfg(feature = "ssr")]
use {
    actix_multipart::Multipart,
    actix_web::{Error, HttpResponse},
    futures_util::StreamExt as _,
    std::io::Write,
};

#[cfg(feature = "ssr")]
pub async fn upload_lighthouse_report(mut payload: Multipart) -> Result<HttpResponse, Error> {
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
