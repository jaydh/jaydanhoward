use axum::{extract::Request, middleware::Next, response::Response};
use axum::http::header::{
    HeaderName, HeaderValue, CONTENT_SECURITY_POLICY, REFERRER_POLICY,
    STRICT_TRANSPORT_SECURITY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};

pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();

    headers.insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; \
             script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval' https://static.cloudflareinsights.com; \
             style-src 'self' 'unsafe-inline'; \
             img-src 'self' https://caddy.jaydanhoward.com data:; \
             media-src 'self' https://caddy.jaydanhoward.com; \
             font-src 'self'; \
             connect-src 'self' https://cloudflareinsights.com; \
             frame-src 'self'; \
             frame-ancestors 'self'; \
             base-uri 'self'; \
             form-action 'self';"
        ),
    );

    headers.insert(
        X_FRAME_OPTIONS,
        HeaderValue::from_static("SAMEORIGIN"),
    );

    headers.insert(
        X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );

    headers.insert(
        STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );

    headers.insert(
        REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(
            "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()"
        ),
    );

    headers.insert(
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    );

    headers.insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );

    headers.insert(
        HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("same-origin"),
    );

    response
}
