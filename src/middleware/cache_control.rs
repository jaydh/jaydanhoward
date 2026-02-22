use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use std::future::{ready, Future, Ready};
use std::pin::Pin;

/// Middleware that adds Cache-Control headers based on file extension
pub struct CacheControl;

impl<S, B> Transform<S, ServiceRequest> for CacheControl
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = CacheControlMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CacheControlMiddleware { service }))
    }
}

pub struct CacheControlMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for CacheControlMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let path = req.path().to_string();

        let fut = self.service.call(req);

        Box::pin(async move {
            let mut res = fut.await?;

            // Determine cache duration based on file extension
            let cache_header = if path.ends_with(".wasm") || path.ends_with(".js") {
                // WASM/JS files: always revalidate via ETag to prevent stale deployments
                "public, no-cache"
            } else if path.ends_with(".woff2")
                || path.ends_with(".woff")
                || path.ends_with(".ttf")
                || path.ends_with(".eot")
                || path.ends_with(".otf")
            {
                // Font files are stable, can cache longer
                "public, max-age=31536000, immutable"
            } else if path.ends_with(".webp")
                || path.ends_with(".png")
                || path.ends_with(".jpg")
                || path.ends_with(".jpeg")
                || path.ends_with(".gif")
                || path.ends_with(".svg")
                || path.ends_with(".ico")
            {
                // Images: 1 week cache with revalidation
                "public, max-age=604800, must-revalidate"
            } else if path.ends_with(".css") {
                // CSS with moderate cache (1 week) and revalidation
                "public, max-age=604800, must-revalidate"
            } else if path.ends_with(".html") || path == "/" {
                // HTML pages should not be cached or cached very short
                "public, max-age=0, must-revalidate"
            } else {
                // Default for other files (1 day)
                "public, max-age=86400"
            };

            // Add Cache-Control header to the response
            res.headers_mut().insert(
                actix_web::http::header::CACHE_CONTROL,
                actix_web::http::header::HeaderValue::from_static(cache_header),
            );

            Ok(res)
        })
    }
}
