use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use std::future::{ready, Future, Ready};
use std::pin::Pin;

/// Middleware that adds security headers to all responses
pub struct SecurityHeaders;

impl<S, B> Transform<S, ServiceRequest> for SecurityHeaders
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = SecurityHeadersMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SecurityHeadersMiddleware { service }))
    }
}

pub struct SecurityHeadersMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for SecurityHeadersMiddleware<S>
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
        let fut = self.service.call(req);

        Box::pin(async move {
            let mut res = fut.await?;

            let headers = res.headers_mut();

            // Content Security Policy - Restricts resource loading to prevent XSS
            // Note: This is a strict policy. Adjust 'unsafe-inline' and 'unsafe-eval' based on your needs.
            // For Leptos hydration, we need 'unsafe-inline' for styles and scripts
            headers.insert(
                actix_web::http::header::CONTENT_SECURITY_POLICY,
                actix_web::http::header::HeaderValue::from_static(
                    "default-src 'self'; \
                     script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; \
                     style-src 'self' 'unsafe-inline'; \
                     img-src 'self' https://caddy.jaydanhoward.com data:; \
                     font-src 'self'; \
                     connect-src 'self'; \
                     frame-ancestors 'none'; \
                     base-uri 'self'; \
                     form-action 'self';"
                ),
            );

            // X-Frame-Options - Prevents clickjacking by disallowing embedding in iframes
            headers.insert(
                actix_web::http::header::X_FRAME_OPTIONS,
                actix_web::http::header::HeaderValue::from_static("DENY"),
            );

            // X-Content-Type-Options - Prevents MIME type sniffing
            headers.insert(
                actix_web::http::header::X_CONTENT_TYPE_OPTIONS,
                actix_web::http::header::HeaderValue::from_static("nosniff"),
            );

            // Strict-Transport-Security (HSTS) - Forces HTTPS for 1 year
            // Note: Only enable this if you're serving over HTTPS
            headers.insert(
                actix_web::http::header::STRICT_TRANSPORT_SECURITY,
                actix_web::http::header::HeaderValue::from_static(
                    "max-age=31536000; includeSubDomains"
                ),
            );

            // Referrer-Policy - Controls how much referrer information is sent
            headers.insert(
                actix_web::http::header::REFERRER_POLICY,
                actix_web::http::header::HeaderValue::from_static("strict-origin-when-cross-origin"),
            );

            // Permissions-Policy - Controls which browser features can be used
            headers.insert(
                actix_web::http::header::HeaderName::from_static("permissions-policy"),
                actix_web::http::header::HeaderValue::from_static(
                    "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()"
                ),
            );

            // X-XSS-Protection - Legacy XSS protection (mostly superseded by CSP)
            headers.insert(
                actix_web::http::header::HeaderName::from_static("x-xss-protection"),
                actix_web::http::header::HeaderValue::from_static("1; mode=block"),
            );

            Ok(res)
        })
    }
}
