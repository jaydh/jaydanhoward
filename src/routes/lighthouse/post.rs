#[cfg(feature = "ssr")]
use {
    actix_multipart::Multipart,
    actix_web::{Error, HttpResponse},
    futures_util::StreamExt as _,
};

#[cfg(feature = "ssr")]
pub async fn upload_lighthouse_report(mut payload: Multipart) -> Result<HttpResponse, Error> {
    // iterate over multipart stream
    while let Some(item) = payload.next().await {
        let mut field = item?;

        // Field in turn is stream of *Bytes* object
        while let Some(chunk) = field.next().await {
            println!("-- CHUNK: \n{:?}", std::str::from_utf8(&chunk?));
        }
    }

    Ok(HttpResponse::Ok().into())
}
