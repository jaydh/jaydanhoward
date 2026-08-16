//! jaydanhoward.com — Foster migration. See
//! .claude/plans/iridescent-skipping-wall.md for the full plan. This file
//! grows milestone by milestone; milestone 2 wires up the chrome + sections
//! that were already proven at full fidelity in the foster PoC
//! (foster/examples/jaydanhoward), now pointed at the real production
//! schema (migrations/ here are byte-identical copies of the real site's).

mod cluster;
mod cluster_audit;
mod conjunction;
mod lighthouse;
mod photography;
mod prometheus_client;
mod request_trace;
mod satellites;
mod security_audit;
mod site_middleware;
mod visitors;

use axum::routing::{get, post};
use axum::{http::StatusCode, Router};
use foster_core::MachineBuilder;
use site_middleware::RateLimiter;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;

async fn health_check() -> StatusCode {
    StatusCode::OK
}

#[tokio::main]
async fn main() {
    // sqlx's rustls-tls feature (and, from milestone 4 on, kube's) needs an
    // explicit process-level crypto provider.
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Real site races 7 algorithms (BFS/DFS/A*/Greedy/Corner/Wall/Random
    // Walk) simultaneously over one shared random grid — all client-side,
    // no server data dependency (same as the real src/components/
    // path_search.rs, which is entirely #[cfg(not(feature = "ssr"))]).
    // Run/pause/reset are plain client state in static/pathfinding.js, not
    // a Foster machine — same reasoning as life.js and theme.js: this is
    // per-visitor UI state (auto-play-on-scroll, like the real site's own
    // local signal), and a Foster machine is one shared instance across
    // every connected client, which would make one visitor's scroll
    // position control play/pause for everyone.

    // Which photo is open in the lightbox used to live here (view/close/
    // next/prev reducers), but that's per-visitor UI state, not shared
    // data — Foster machines are one shared instance across every
    // connected client, so one visitor opening a photo would pop it open
    // in everyone else's lightbox too. Moved entirely to static/
    // photography.js (same reasoning as theme/life/pathfinding); this
    // machine now only holds the real, shared photo list.
    let photography = MachineBuilder::new("photography", "loaded", photography::fetch_photos())
        .template(include_str!("../static/index.html"))
        .build();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:foster@localhost:5433/jaydanhoward".to_string());
    let pg_pool = visitors::create_pool(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    let visitors_machine = {
        let pool_for_reducer = pg_pool.clone();
        MachineBuilder::new("visitors", "loaded", visitors::fetch_visitor_stats(&pg_pool))
            .on("loaded", "refresh", "loaded", move |_ctx, _payload| {
                Ok(visitors::fetch_visitor_stats(&pool_for_reducer))
            })
            .build()
    };

    // Lighthouse "Load Report" gate: whether it's open is per-visitor UI
    // state, not shared data, so it's plain client-side state in
    // static/lighthouse.js rather than a Foster machine (same reasoning as
    // theme/life/pathfinding/photography's lightbox). The report content
    // itself comes from an external CI job POSTing to /api/lighthouse
    // (src/lighthouse.rs, real Basic-Auth-protected upload endpoint ported
    // verbatim from routes/lighthouse/post.rs) — not a live self-audit.

    // Full Prometheus-backed "Homelab Cluster" panel (10+ real panels — see
    // cluster.rs) is a genuine server-push live feed, not a Foster machine —
    // Foster machines are one shared instance across every connected client,
    // which is fine for data that really is global (like this), but here it
    // matches the real site's actual transport (a true EventSource/SSE
    // stream ticking once a second, GET /api/metrics/stream) rather than
    // request/response polling through Foster's reducer model. See
    // static/cluster.js for the client-side consumer.
    let mut machines = HashMap::new();
    machines.insert("photography".to_string(), photography);
    machines.insert("visitors".to_string(), visitors_machine);

    // Real conjunction screening (Hoots + SGP4 + TCA + rayon — see
    // conjunction.rs). Foster's role is deliberately tiny, same shape as
    // the earlier PoC: just the button's idle/started label. The real
    // screening pass and its results are a background job persisted to
    // the real conjunction_screenings/conjunction_events tables, polled
    // independently of Foster's own SSE for this machine.
    let conjunction_machine = MachineBuilder::new("conjunction", "idle", serde_json::json!({}))
        .state("started")
        .pass("idle", "start_screening", "started")
        .pass("started", "start_screening", "started")
        .build();
    machines.insert("conjunction".to_string(), conjunction_machine);

    // Real 3D satellite tracking — see satellites.rs for the full rationale.
    // Foster only owns the run/pause + playback-speed labels (small,
    // discrete state); the background propagation loop and the WebGL2
    // rendering pipeline live outside Foster entirely (a shared tokio task
    // and static/satellites.js respectively), same shape as conjunction.
    let satellites_runtime = std::sync::Arc::new(satellites::SatellitesRuntime::new());
    satellites::spawn_background_loop(satellites_runtime.clone(), Some(pg_pool.clone()));

    // Daily LLM cluster audit — second opinion on top of the Prometheus
    // threshold alerts. Skipped when Prometheus or the Anthropic key aren't
    // configured. Replicas race for the day's bucket via
    // cluster_audit_claims (see cluster_audit.rs) so only one of them
    // actually calls Claude. Ported from src/startup.rs.
    if std::env::var("PROMETHEUS_URL").is_ok() && std::env::var("ANTHROPIC_API_KEY").is_ok() {
        let pool_for_audit = pg_pool.clone();
        tokio::spawn(async move {
            const AUDIT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

            // Spread pod startup across up to 10 minutes so replicas don't
            // burst the claim race (and Anthropic) simultaneously. No rand
            // dependency in this crate — nanosecond-of-boot is jitter enough.
            let jitter_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as u64 % 600)
                .unwrap_or(0);
            tokio::time::sleep(Duration::from_secs(jitter_secs)).await;

            loop {
                if cluster_audit::try_claim_cluster_audit(&pool_for_audit).await {
                    match cluster_audit::run_audit(&pool_for_audit).await {
                        Ok((summary, significance)) => {
                            println!("Cluster audit complete (significance={significance}/10): {summary}");
                        }
                        Err(e) => eprintln!("Cluster audit failed: {e}"),
                    }
                }
                tokio::time::sleep(AUDIT_INTERVAL).await;
            }
        });
    }
    let satellites_machine = {
        let running_for_pause = satellites_runtime.running.clone();
        let running_for_resume = satellites_runtime.running.clone();
        let steps_up_running = satellites_runtime.steps_per_tick.clone();
        let steps_up_paused = satellites_runtime.steps_per_tick.clone();
        let steps_down_running = satellites_runtime.steps_per_tick.clone();
        let steps_down_paused = satellites_runtime.steps_per_tick.clone();

        fn steps_ctx(steps: u32) -> serde_json::Value {
            serde_json::json!({ "steps_per_tick": steps })
        }

        MachineBuilder::new(
            "satellites",
            "running",
            serde_json::json!({ "steps_per_tick": 12 }),
        )
        .state("paused")
        .on("running", "toggle_run", "paused", move |ctx, _| {
            running_for_pause.store(false, std::sync::atomic::Ordering::Relaxed);
            Ok(ctx)
        })
        .on("paused", "toggle_run", "running", move |ctx, _| {
            running_for_resume.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(ctx)
        })
        .on("running", "speed_up", "running", move |_ctx, _| {
            let next = (steps_up_running.load(std::sync::atomic::Ordering::Relaxed) * 2).min(96);
            steps_up_running.store(next, std::sync::atomic::Ordering::Relaxed);
            Ok(steps_ctx(next))
        })
        .on("paused", "speed_up", "paused", move |_ctx, _| {
            let next = (steps_up_paused.load(std::sync::atomic::Ordering::Relaxed) * 2).min(96);
            steps_up_paused.store(next, std::sync::atomic::Ordering::Relaxed);
            Ok(steps_ctx(next))
        })
        .on("running", "speed_down", "running", move |_ctx, _| {
            let cur = steps_down_running.load(std::sync::atomic::Ordering::Relaxed);
            let next = (cur / 2).max(1);
            steps_down_running.store(next, std::sync::atomic::Ordering::Relaxed);
            Ok(steps_ctx(next))
        })
        .on("paused", "speed_down", "paused", move |_ctx, _| {
            let cur = steps_down_paused.load(std::sync::atomic::Ordering::Relaxed);
            let next = (cur / 2).max(1);
            steps_down_paused.store(next, std::sync::atomic::Ordering::Relaxed);
            Ok(steps_ctx(next))
        })
        .build()
    };
    machines.insert("satellites".to_string(), satellites_machine);

    let pkg_dir = "/app/pkg";
    let pkg_dir = if std::path::Path::new(pkg_dir).exists() {
        pkg_dir.to_string()
    } else {
        // CI's "Build foster-client WASM" step (general.yml) and local dev
        // both produce this at <repo-root>/foster/pkg (also why .gitignore
        // has a bare `pkg` entry) — one level up from foster-server, not
        // two. Was previously off by one level, which made this fallback
        // silently 404 every /pkg/* request in CI (never in production,
        // which always hits the /app/pkg branch above via the Dockerfile).
        concat!(env!("CARGO_MANIFEST_DIR"), "/../foster/pkg").to_string()
    };
    // The compile-time CARGO_MANIFEST_DIR baked in by concat! is the
    // Docker builder stage's path (/build), which doesn't exist in the
    // final distroless runtime image — only /app/static does (see
    // Dockerfile's final COPY). Same fallback shape as pkg_dir above; a
    // real deploy silently 404s on every JS asset without this (only
    // caught by an actual in-cluster deploy, not local `cargo run`, since
    // CARGO_MANIFEST_DIR happens to still be a valid path there).
    let static_dir = "/app/static";
    let static_dir = if std::path::Path::new(static_dir).exists() {
        static_dir.to_string()
    } else {
        concat!(env!("CARGO_MANIFEST_DIR"), "/static").to_string()
    };

    let http_client = reqwest::Client::new();
    let world_map_svg = std::sync::Arc::new(visitors::fetch_world_map_svg(&http_client).await);

    let trace_router: Router = Router::new()
        .route("/api/request-trace", get(request_trace::get_request_trace));

    let conjunction_router = Router::new()
        .route("/api/conjunction", get(conjunction::get_screening))
        .route("/api/conjunction/start", post(conjunction::start_screening))
        .with_state(conjunction::ConjunctionAppState {
            screening: conjunction::initial_state(),
            pool: pg_pool.clone(),
        });

    let satellites_router = Router::new()
        .route("/api/satellites", get(satellites::get_positions))
        .with_state(satellites_runtime);

    let world_map_router = {
        let svg = world_map_svg.clone();
        Router::new().route(
            "/world-map.svg",
            get(move || {
                let svg = svg.clone();
                async move {
                    (
                        [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
                        (*svg).clone(),
                    )
                }
            }),
        )
    };

    // Rate limiter for the two Basic-Auth upload endpoints: 5 requests per
    // minute each, same as the real site's lighthouse-only limiter (now
    // shared across both upload routes rather than duplicated).
    let auth_rate_limiter = RateLimiter::new(5, Duration::from_secs(60));
    let lighthouse_limiter = auth_rate_limiter.clone();
    let security_audit_limiter = auth_rate_limiter.clone();
    let claude_audit_limiter = auth_rate_limiter.clone();

    let app = foster_server::router(machines)
        .merge(trace_router)
        .merge(world_map_router)
        .merge(conjunction_router)
        .merge(satellites_router)
        .route(
            "/api/lighthouse",
            post(lighthouse::upload_lighthouse_report).layer(axum::middleware::from_fn(move |req, next| {
                let limiter = lighthouse_limiter.clone();
                async move { limiter.check_middleware(req, next).await }
            })),
        )
        .route(
            "/api/security-audit",
            post(security_audit::upload_security_audit)
                .layer(axum::middleware::from_fn(move |req, next| {
                    let limiter = security_audit_limiter.clone();
                    async move { limiter.check_middleware(req, next).await }
                }))
                .with_state(pg_pool.clone()),
        )
        .route(
            "/api/audit/claude",
            post(cluster::ingest_claude_audit)
                .layer(axum::middleware::from_fn(move |req, next| {
                    let limiter = claude_audit_limiter.clone();
                    async move { limiter.check_middleware(req, next).await }
                }))
                .with_state(pg_pool.clone()),
        )
        .route(
            "/api/metrics/stream",
            get(cluster::metrics_stream).with_state(pg_pool.clone()),
        )
        .route("/health_check", get(health_check))
        .nest_service("/pkg", ServeDir::new(pkg_dir))
        .fallback_service(ServeDir::new(static_dir))
        .layer(axum::middleware::from_fn_with_state(
            pg_pool,
            visitors::visitor_logger,
        ))
        .layer(axum::middleware::from_fn(site_middleware::cache_control))
        .layer(axum::middleware::from_fn(site_middleware::security_headers))
        .layer(CompressionLayer::new());

    let addr: SocketAddr = "0.0.0.0:8000".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("jaydanhoward (Foster) → http://{addr}");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
