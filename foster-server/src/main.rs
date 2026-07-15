//! jaydanhoward.com — Foster migration. See
//! .claude/plans/iridescent-skipping-wall.md for the full plan. This file
//! grows milestone by milestone; milestone 2 wires up the chrome + sections
//! that were already proven at full fidelity in the foster PoC
//! (foster/examples/jaydanhoward), now pointed at the real production
//! schema (migrations/ here are byte-identical copies of the real site's).

mod cluster;
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

    let theme = MachineBuilder::new("theme", "light", serde_json::json!({}))
        .state("dark")
        .pass("light", "toggle_theme", "dark")
        .pass("dark", "toggle_theme", "light")
        .build();

    let nav = MachineBuilder::new("nav", "closed", serde_json::json!({}))
        .state("open")
        .pass("closed", "toggle_contact", "open")
        .pass("open", "toggle_contact", "closed")
        .pass("open", "close_contact", "closed")
        .template(include_str!("../static/index.html"))
        .build();

    let life = MachineBuilder::new("life", "paused", serde_json::json!({ "reset_nonce": 0 }))
        .state("running")
        .pass("paused", "toggle_run", "running")
        .pass("running", "toggle_run", "paused")
        .on("paused", "reset", "paused", |ctx, _| {
            let n = ctx["reset_nonce"].as_i64().unwrap_or(0);
            Ok(serde_json::json!({ "reset_nonce": n + 1 }))
        })
        .on("running", "reset", "running", |ctx, _| {
            let n = ctx["reset_nonce"].as_i64().unwrap_or(0);
            Ok(serde_json::json!({ "reset_nonce": n + 1 }))
        })
        .build();

    let pathfinding = MachineBuilder::new(
        "pathfinding",
        "paused",
        serde_json::json!({ "algorithm": "bfs", "reset_nonce": 0 }),
    )
    .state("running")
    .pass("paused", "toggle_run", "running")
    .pass("running", "toggle_run", "paused")
    .on("paused", "select_bfs", "paused", |ctx, _| {
        let n = ctx["reset_nonce"].as_i64().unwrap_or(0);
        Ok(serde_json::json!({ "algorithm": "bfs", "reset_nonce": n + 1 }))
    })
    .on("running", "select_bfs", "running", |ctx, _| {
        let n = ctx["reset_nonce"].as_i64().unwrap_or(0);
        Ok(serde_json::json!({ "algorithm": "bfs", "reset_nonce": n + 1 }))
    })
    .on("paused", "select_astar", "paused", |ctx, _| {
        let n = ctx["reset_nonce"].as_i64().unwrap_or(0);
        Ok(serde_json::json!({ "algorithm": "astar", "reset_nonce": n + 1 }))
    })
    .on("running", "select_astar", "running", |ctx, _| {
        let n = ctx["reset_nonce"].as_i64().unwrap_or(0);
        Ok(serde_json::json!({ "algorithm": "astar", "reset_nonce": n + 1 }))
    })
    .on("paused", "reset", "paused", |ctx, _| {
        let n = ctx["reset_nonce"].as_i64().unwrap_or(0);
        Ok(serde_json::json!({ "algorithm": ctx["algorithm"], "reset_nonce": n + 1 }))
    })
    .on("running", "reset", "running", |ctx, _| {
        let n = ctx["reset_nonce"].as_i64().unwrap_or(0);
        Ok(serde_json::json!({ "algorithm": ctx["algorithm"], "reset_nonce": n + 1 }))
    })
    .build();

    let photography = {
        let initial = photography::fetch_photos();
        let photos_for_reducers = initial["photos"].clone();
        let photo_count = photos_for_reducers.as_array().map(|a| a.len()).unwrap_or(0) as i64;

        fn set_viewing(mut ctx: serde_json::Value, photos: &serde_json::Value, index: i64) -> serde_json::Value {
            ctx["viewing_index"] = serde_json::json!(index);
            if index >= 0 {
                if let Some(photo) = photos.as_array().and_then(|a| a.get(index as usize)) {
                    ctx["viewing_url"] = photo["medium_url"].clone();
                    ctx["viewing_name"] = photo["name"].clone();
                }
            } else {
                ctx["viewing_url"] = serde_json::json!("");
                ctx["viewing_name"] = serde_json::json!("");
            }
            ctx
        }

        let photos_1 = photos_for_reducers.clone();
        let photos_2 = photos_for_reducers.clone();
        let photos_3 = photos_for_reducers.clone();

        MachineBuilder::new("photography", "loaded", initial)
            .on("loaded", "view", "loaded", move |ctx, payload| {
                let i = payload.get("index").and_then(|v| v.as_i64()).unwrap_or(-1);
                Ok(set_viewing(ctx, &photos_1, i))
            })
            .on("loaded", "close", "loaded", |ctx, _| Ok(set_viewing(ctx, &serde_json::Value::Null, -1)))
            .on("loaded", "next", "loaded", move |ctx, _| {
                let i = ctx["viewing_index"].as_i64().unwrap_or(-1);
                let next = if photo_count > 0 && i >= 0 { (i + 1) % photo_count } else { i };
                Ok(set_viewing(ctx, &photos_2, next))
            })
            .on("loaded", "prev", "loaded", move |ctx, _| {
                let i = ctx["viewing_index"].as_i64().unwrap_or(-1);
                let prev = if photo_count > 0 && i >= 0 { (i - 1 + photo_count) % photo_count } else { i };
                Ok(set_viewing(ctx, &photos_3, prev))
            })
            .build()
    };

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

    // Lighthouse "Load Report" gate — matches the real
    // src/components/dev.rs::Lighthouse component's lazy-iframe UX exactly
    // (a button that reveals the iframe on click, nothing more). The report
    // content itself comes from an external CI job POSTing to
    // /api/lighthouse (src/lighthouse.rs, real Basic-Auth-protected upload
    // endpoint ported verbatim from routes/lighthouse/post.rs) — not a live
    // self-audit.
    let lighthouse_machine = MachineBuilder::new("lighthouse", "hidden", serde_json::json!({}))
        .state("loaded")
        .pass("hidden", "load_report", "loaded")
        .build();

    // Full Prometheus-backed cluster panel (10+ real panels — see
    // cluster.rs). PROMETHEUS_URL is unset here (unreachable from this dev
    // laptop), so every metric gracefully degrades to zero locally; live
    // verification happens in milestone 8's in-cluster staging step. GitOps
    // + backup Job status (kube) and network insights/spike config/Claude
    // audit log (Postgres) are all real right now, same as before.
    let cluster_machine = {
        let pool_for_reducer = pg_pool.clone();
        MachineBuilder::new("cluster", "loaded", cluster::fetch_cluster_data(&pg_pool))
            .on("loaded", "refresh", "loaded", move |_ctx, _payload| {
                Ok(cluster::fetch_cluster_data(&pool_for_reducer))
            })
            .build()
    };

    let mut machines = HashMap::new();
    machines.insert("theme".to_string(), theme);
    machines.insert("nav".to_string(), nav);
    machines.insert("life".to_string(), life);
    machines.insert("pathfinding".to_string(), pathfinding);
    machines.insert("photography".to_string(), photography);
    machines.insert("visitors".to_string(), visitors_machine);
    machines.insert("lighthouse".to_string(), lighthouse_machine);
    machines.insert("cluster".to_string(), cluster_machine);

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
    satellites::spawn_background_loop(satellites_runtime.clone());
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
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../foster/pkg").to_string()
    };
    let static_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/static");

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
            post(cluster::ingest_claude_audit).with_state(pg_pool.clone()),
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
