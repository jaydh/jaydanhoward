use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpResponse,
};
use std::collections::HashMap;
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Simple in-memory rate limiter using a token bucket approach
#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<RateLimitState>>,
    max_requests: usize,
    window: Duration,
}

struct RateLimitState {
    // Map of IP address to (request_count, window_start)
    clients: HashMap<String, (usize, Instant)>,
    last_cleanup: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `max_requests` - Maximum number of requests allowed per window
    /// * `window` - Time window for rate limiting
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(RateLimitState {
                clients: HashMap::new(),
                last_cleanup: Instant::now(),
            })),
            max_requests,
            window,
        }
    }

    fn check_rate_limit(&self, ip: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();

        // Cleanup old entries every 5 minutes to prevent unbounded memory growth
        if now.duration_since(state.last_cleanup) > Duration::from_secs(300) {
            state.clients.retain(|_, (_, start)| {
                now.duration_since(*start) < self.window
            });
            state.last_cleanup = now;
        }

        // Get or create client entry
        let entry = state
            .clients
            .entry(ip.to_string())
            .or_insert((0, now));

        // Check if we're in a new window
        if now.duration_since(entry.1) >= self.window {
            entry.0 = 1;
            entry.1 = now;
            return true;
        }

        // Check if we've exceeded the rate limit
        if entry.0 >= self.max_requests {
            return false;
        }

        // Increment counter
        entry.0 += 1;
        true
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimiter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimiterMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimiterMiddleware {
            service,
            limiter: self.clone(),
        }))
    }
}

pub struct RateLimiterMiddleware<S> {
    service: S,
    limiter: RateLimiter,
}

impl<S, B> Service<ServiceRequest> for RateLimiterMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Extract client IP address
        let ip = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();

        // Check rate limit
        if !self.limiter.check_rate_limit(&ip) {
            // Rate limit exceeded
            let response = HttpResponse::TooManyRequests()
                .body("Too many requests. Please try again later.");
            let service_response = req.into_response(response).map_into_right_body();
            return Box::pin(async move { Ok(service_response) });
        }

        // Continue with the request
        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
            Ok(res.map_into_left_body())
        })
    }
}
