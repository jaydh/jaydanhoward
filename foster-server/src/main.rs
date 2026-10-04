//! jaydanhoward.com — Foster migration. See
//! .claude/plans/iridescent-skipping-wall.md for the full plan. This file
//! grows milestone by milestone; milestone 2 wires up the chrome + sections
//! that were already proven at full fidelity in the foster PoC
//! (foster/examples/jaydanhoward), now pointed at the real production
//! schema (migrations/ here are byte-identical copies of the real site's).

mod cluster;
mod cluster_audit;
mod cluster_view;
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
use futures_util::StreamExt;
use std::net::SocketAddr;
use std::sync::Arc;
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

    // Game of Life and Pathfinding are entirely client-side WebGL sims with
    // no server data: Rust crates in ../widgets, built to WASM and
    // lazy-loaded by Foster's `fx-widget` when they scroll into view.

    // Gallery + lightbox, as a local (per-visitor, in-browser) machine: the
    // photo list is fetched once at startup and never changes, so it ships
    // embedded in the page, and which photo is open is per-visitor state.
    // "open" merges the clicked tile's item (incl. its index) into context;
    // prev/next step through the list; the lightbox binds ctx:medium_url.
    let photography = MachineBuilder::new("photography", "grid", photography::fetch_photos())
        .merge("grid", "open", "viewing")
        .step("viewing", "next", "viewing", "photos", "index", 1)
        .step("viewing", "prev", "viewing", "photos", "index", -1)
        .pass("viewing", "close", "grid")
        .local()
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

    // Lighthouse "Load Report" gate: the local "lighthouse" machine below.
    // The report content itself comes from an external CI job POSTing to /api/lighthouse
    // (src/lighthouse.rs, real Basic-Auth-protected upload endpoint ported
    // verbatim from routes/lighthouse/post.rs) — not a live self-audit.

    let mut machines = HashMap::new();
    machines.insert("photography".to_string(), photography);
    machines.insert("visitors".to_string(), visitors_machine);

    // Per-visitor UI state — `.local()` machines run in the browser (Foster
    // embeds their definitions in the page), so one visitor's toggle never
    // reaches anyone else and costs no round trip.
    //
    // theme: on <html>, `fx-class="dark:dark"`. `.persist()` keeps the choice
    // in localStorage; on a first visit the <head> script picks light/dark
    // from prefers-color-scheme and stamps data-fx-state before first paint.
    let theme = MachineBuilder::new("theme", "light", serde_json::json!({}))
        .pass("light", "toggle", "dark")
        .pass("dark", "toggle", "light")
        .persist()
        .build();
    machines.insert("theme".to_string(), theme);
    // contact: the nav dropdown; `click@outside` closes it.
    let contact = MachineBuilder::new("contact", "closed", serde_json::json!({}))
        .pass("closed", "toggle", "open")
        .pass("open", "toggle", "closed")
        .pass("open", "close", "closed")
        .local()
        .build();
    machines.insert("contact".to_string(), contact);
    // lighthouse: the iframe ships without a src (an iframe inside a
    // display:none wrapper still loads, so a real src would pull the ~680KB
    // report on every page load). "load" — from the button, or `visible`
    // once scrolled to — merges `report_src` into context, which
    // fx-bind-attr copies onto the iframe.
    let lighthouse = MachineBuilder::new("lighthouse", "gate", serde_json::json!({}))
        .merge("gate", "load", "loaded")
        .local()
        .build();
    machines.insert("lighthouse".to_string(), lighthouse);
    // spy: which section the nav highlights. Each <main> fires its own id
    // via fx-on="enter->…" as it crosses the reading line; links use
    // fx-class="<id>:active". On .page, so the nav's theme toggle addresses
    // its machine explicitly (fx-on="click->theme:toggle").
    let spy = {
        const SECTIONS: [&str; 7] = ["about", "trace", "cluster", "satellites", "life", "path", "photography"];
        let mut b = MachineBuilder::new("spy", "about", serde_json::json!({}));
        for from in SECTIONS {
            for to in SECTIONS {
                b = b.pass(from, to, to);
            }
        }
        b.local().build()
    };
    machines.insert("spy".to_string(), spy);

    // Real conjunction screening (Hoots + SGP4 + TCA + rayon — see
    // conjunction.rs). Foster's role is deliberately tiny, same shape as
    // the earlier PoC: just the button's idle/started label. The real
    // screening pass and its results are a background job persisted to
    // the real conjunction_screenings/conjunction_events tables, polled
    // independently of Foster's own SSE for this machine.
    //
    // Shared: there's one screening job for everyone, so every viewer sees
    // the same status. "start" kicks off the background job; a feed (below)
    // pushes its status every 2s while someone is watching.
    let conjunction_state = conjunction::ConjunctionAppState {
        screening: conjunction::initial_state(),
        pool: pg_pool.clone(),
    };
    let conjunction_last = Arc::new(conjunction::latest_from_db(&pg_pool).await);
    let conjunction_machine = {
        let state = conjunction_state.clone();
        MachineBuilder::new("conjunction", "live", conjunction::screening_view(&conjunction_last))
            .on("live", "status", "live", |_, view| Ok(view))
            .on("live", "start", "live", move |_, _| {
                conjunction::start(&state);
                Ok(conjunction::screening_view(&serde_json::json!({ "status": "running" })))
            })
            .shared()
            .build()
    };
    machines.insert("conjunction".to_string(), conjunction_machine);

    // Homelab cluster card: shared, fed once a second (while watched) by the
    // cluster feed below. Each tick replaces the whole display model.
    let cluster_machine = MachineBuilder::new("cluster", "live", cluster_view::cluster_view(&serde_json::json!({}), ""))
        .on("live", "tick", "live", |_, view| Ok(view))
        .shared()
        .build();
    machines.insert("cluster".to_string(), cluster_machine);

    // "How You Got Here": per-visitor, filled by the request_event below on
    // every page load and Refresh click.
    let request_trace_machine = MachineBuilder::new("request_trace", "pending", serde_json::json!({}))
        .on("pending", "trace", "traced", |_, view| Ok(view))
        .on("traced", "trace", "traced", |_, view| Ok(view))
        .build();
    machines.insert("request_trace".to_string(), request_trace_machine);

    // Real 3D satellite tracking — see satellites.rs for the full rationale.
    // The shared "satellites" machine owns run/pause + playback speed; the
    // propagation loop is a shared tokio task, and the globe is the Rust
    // WebGL widget in ../widgets/satellites.
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

        /// Context for a sim speed: each tick advances `steps` 5-minute steps,
        /// labeled as sim time per real second.
        fn steps_ctx(steps: u32) -> serde_json::Value {
            let sim_min_per_sec = steps as f64 * 5.0;
            let label = if sim_min_per_sec < 60.0 {
                format!("{sim_min_per_sec:.0}m/s")
            } else {
                format!("{:.1}h/s", sim_min_per_sec / 60.0)
            };
            serde_json::json!({ "steps_per_tick": steps, "speed_label": label })
        }

        // Shared: it drives the one server-side simulation (running flag +
        // speed are process-wide atomics), so every visitor sees the same
        // run/pause state and speed.
        MachineBuilder::new("satellites", "running", steps_ctx(12))
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
        .shared()
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

    // Feeds only poll while someone has the machine's live stream open.
    let every = |secs: u64| {
        let mut interval = tokio::time::interval(Duration::from_secs(secs));
        // After an idle stretch (no viewers), resume on the next tick rather
        // than bursting through every missed one.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        tokio_stream::wrappers::IntervalStream::new(interval)
    };
    let cluster_feed = {
        let pool = pg_pool.clone();
        every(1).then(move |_| {
            let pool = pool.clone();
            async move {
                let snapshot = cluster::fetch_cluster_snapshot(&pool).await;
                let updated = chrono::Utc::now().format("%H:%M:%S UTC").to_string();
                cluster_view::cluster_view(&snapshot, &updated)
            }
        })
    };
    let conjunction_feed = every(2).then(move |_| {
        let state = conjunction_state.clone();
        let last = conjunction_last.clone();
        async move { conjunction::screening_view(&conjunction::current(&state, &last).await) }
    });

    let app = foster_server::Foster::new(machines)
        .feed("cluster", "tick", cluster_feed)
        .feed("conjunction", "status", conjunction_feed)
        .request_event("request_trace", "trace", |req: foster_server::RequestInfo| async move {
            request_trace::trace_view(&request_trace::trace(&req.headers, req.remote_addr).await)
        })
        .router()
        .merge(world_map_router)
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
