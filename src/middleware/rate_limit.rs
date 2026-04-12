use axum::{extract::Request, http::StatusCode, middleware::Next, response::{IntoResponse, Response}};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<RateLimitState>>,
    max_requests: usize,
    window: Duration,
}

struct RateLimitState {
    clients: HashMap<String, (usize, Instant)>,
    last_cleanup: Instant,
}

impl RateLimiter {
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

        if now.duration_since(state.last_cleanup) > Duration::from_secs(300) {
            state.clients.retain(|_, (_, start)| {
                now.duration_since(*start) < self.window
            });
            state.last_cleanup = now;
        }

        let entry = state
            .clients
            .entry(ip.to_string())
            .or_insert((0, now));

        if now.duration_since(entry.1) >= self.window {
            entry.0 = 1;
            entry.1 = now;
            return true;
        }

        if entry.0 >= self.max_requests {
            return false;
        }

        entry.0 += 1;
        true
    }

    pub async fn check_middleware(&self, req: Request, next: Next) -> Response {
        let ip = req
            .headers()
            .get("x-real-ip")
            .or_else(|| req.headers().get("x-forwarded-for"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        if !self.check_rate_limit(&ip) {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                "Too many requests. Please try again later.",
            )
                .into_response();
        }

        next.run(req).await
    }
}
