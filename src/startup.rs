#[cfg(feature = "ssr")]
pub async fn run() -> Result<(), std::io::Error> {
    use crate::components::conjunction::ConjunctionCache;
    use crate::components::satellite_tracker::TleCache;
    use crate::components::App;
    use crate::db::create_pool;
    use crate::middleware::cache_control::CacheControl;
    use crate::middleware::rate_limit::RateLimiter;
    use crate::middleware::security_headers::SecurityHeaders;
    use crate::middleware::visitor_logger::VisitorLogger;
    use crate::routes::{
        fetch_world_map_svg, health_check, metrics_stream, robots_txt, upload_lighthouse_report,
        world_map, WorldMapSvg,
    };
    use crate::telemtry::{get_subscriber, init_subscriber};
    use actix_files::Files;
    use actix_web::{web, HttpServer};
    use leptos::prelude::*;
    use leptos_actix::{generate_route_list, LeptosRoutes};
    use leptos_meta::MetaTags;
    use runfiles::{rlocation, Runfiles};
    use std::time::Duration;
    use tracing::log;

    let subscriber = get_subscriber("jaydanhoward".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    console_error_panic_hook::set_once();

    let r = Runfiles::create().expect("Must run using bazel with runfiles");
    let leptos_toml_path = rlocation!(r, "_main/leptos.toml").expect("Failed to locate runfile");
    let assets_root = leptos_toml_path
        .parent()
        .expect("Failed to locate assets root")
        .to_path_buf();
    let wasm_dir = rlocation!(r, "_main/jaydanhoward_wasm/jaydanhoward_wasm.js")
        .expect("Failed to locate WASM output")
        .parent()
        .expect("Failed to locate WASM dir")
        .to_path_buf();

    log::info!("assets_root={:?} exists={}", assets_root, assets_root.exists());
    log::info!("wasm_dir={:?} exists={}", wasm_dir, wasm_dir.exists());

    let conf = get_configuration(Some(leptos_toml_path.to_string_lossy().as_ref()))
        .expect("Failed to read conf");

    let addr = conf.leptos_options.site_addr;

    // Optionally connect to Postgres if DATABASE_URL is set
    let pool = match std::env::var("DATABASE_URL") {
        Ok(url) => match create_pool(&url).await {
            Ok(p) => {
                log::info!("Connected to Postgres");
                Some(p)
            }
            Err(e) => {
                log::warn!("Failed to connect to Postgres: {}", e);
                None
            }
        },
        Err(_) => {
            log::info!("DATABASE_URL not set, visitor tracking disabled");
            None
        }
    };

    let http_client = reqwest::Client::new();

    log::info!("Fetching world map from Natural Earth...");
    let world_map_svg = fetch_world_map_svg(&http_client).await;
    log::info!("World map ready ({} bytes)", world_map_svg.len());

    let world_map_data = web::Data::new(WorldMapSvg(world_map_svg));
    let tle_cache = web::Data::new(TleCache::new(std::collections::HashMap::new()));
    let conjunction_cache =
        web::Data::new(ConjunctionCache::new(std::collections::HashMap::new()));

    // Startup conjunction screening — chunk-based distributed approach.
    //
    // Two phases run concurrently on every pod:
    //
    //   Coordinator (one task per group): races to claim the group via the Postgres
    //   distributed lock.  The winner fetches TLEs, creates chunk rows in the DB,
    //   and stores TLEs in the local TleCache.  Losers return immediately.
    //
    //   Worker (one task per pod): loops over claim_next_chunk(), fetching TLEs
    //   on-demand if not already cached, then calls screen_chunk().  Exits after
    //   10 consecutive seconds with no claimable chunks.
    //
    // Group sizes vary by 5 orders of magnitude (gps-ops: 496 pairs vs active: 104M),
    // so distributing by chunk rather than by group keeps pods evenly loaded.
    {
        use crate::components::satellite_tracker::TleData;

        const CHUNK_SIZE: usize = 500; // satellites per chunk; ~500 sats × ~14k = ~7M pairs
        const GROUPS: [&str; 5] = ["stations", "gps-ops", "visual", "active", "starlink"];

        let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());

        /// Parse a CelesTrak TLE text body into a Vec<TleData>.
        fn parse_tles(text: &str) -> Vec<TleData> {
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            let mut tles = Vec::new();
            for chunk in lines.chunks(3) {
                if chunk.len() == 3
                    && chunk[1].trim_start().starts_with("1 ")
                    && chunk[2].trim_start().starts_with("2 ")
                {
                    tles.push(TleData {
                        name:  chunk[0].trim().to_string(),
                        line1: chunk[1].trim().to_string(),
                        line2: chunk[2].trim().to_string(),
                    });
                }
            }
            tles
        }

        /// Fetch TLEs for a group from CelesTrak; returns None on any error.
        async fn fetch_tles(
            client: &reqwest::Client,
            group: &str,
        ) -> Option<Vec<TleData>> {
            let url = format!(
                "https://celestrak.org/NORAD/elements/gp.php?GROUP={group}&FORMAT=tle"
            );
            let resp = client
                .get(&url)
                .timeout(Duration::from_secs(60))
                .send()
                .await
                .ok()
                .filter(|r| r.status().is_success())?;
            let text = resp.text().await.ok()?;
            let tles = parse_tles(&text);
            if tles.is_empty() { None } else { Some(tles) }
        }

        // ── Coordinator tasks ────────────────────────────────────────
        for group in GROUPS {
            let pool_opt   = pool.clone().map(web::Data::new);
            let tle_cache  = tle_cache.clone();
            let client     = http_client.clone();
            let hostname   = hostname.clone();

            tokio::spawn(async move {
                // Only the winning pod creates the screening + chunks.
                let screening_id = match pool_opt.as_ref() {
                    None => return, // no DB → skip (worker also exits early)
                    Some(p) => match crate::db::try_claim_conjunction_startup(
                        p, group, &hostname, 60,
                    ).await {
                        Ok(Some(id)) => id,
                        Ok(None) => {
                            log::debug!("Coordinator: group={group} already claimed, skipping");
                            return;
                        }
                        Err(e) => {
                            log::warn!("Coordinator: DB claim failed for group={group}: {e}");
                            return;
                        }
                    },
                };

                log::info!("Coordinator: fetching TLEs for group={group}");
                let tles = match fetch_tles(&client, group).await {
                    Some(t) => t,
                    None => {
                        log::warn!("Coordinator: TLE fetch failed for group={group}");
                        return;
                    }
                };

                let n = tles.len();
                let total_pairs = n * n.saturating_sub(1) / 2;
                log::info!("Coordinator: {n} TLEs ({total_pairs} pairs) for group={group}");

                let p = pool_opt.as_ref().unwrap();
                if let Err(e) =
                    crate::db::update_conjunction_total_pairs(p, screening_id, total_pairs as i64)
                        .await
                {
                    log::warn!("Coordinator: total_pairs update failed for {group}: {e}");
                }

                match crate::db::create_chunks(p, screening_id, group, n, CHUNK_SIZE).await {
                    Ok(k) => log::info!("Coordinator: created {k} chunks for group={group}"),
                    Err(e) => {
                        log::error!("Coordinator: create_chunks failed for {group}: {e}");
                        return;
                    }
                }

                // Cache TLEs locally so this pod's worker avoids a redundant fetch.
                tle_cache
                    .write()
                    .await
                    .insert(group.to_string(), (std::time::Instant::now(), tles));
            });
        }

        // ── Worker loop ──────────────────────────────────────────────
        {
            let pool_opt          = pool.clone().map(web::Data::new);
            let tle_cache         = tle_cache.clone();
            let client            = http_client.clone();
            let hostname          = hostname.clone();

            tokio::spawn(async move {
                use crate::components::conjunction::screen_chunk;

                let pool = match pool_opt {
                    None    => return, // no DB → nothing to claim
                    Some(p) => p,
                };

                // Small delay so coordinator tasks start their TLE fetches before
                // the worker tries to claim (avoids a spurious idle timeout).
                tokio::time::sleep(Duration::from_millis(500)).await;

                let mut consecutive_idle: u32 = 0;

                loop {
                    match crate::db::claim_next_chunk(&pool, &hostname).await {
                        Ok(Some(chunk)) => {
                            consecutive_idle = 0;
                            let group = chunk.group_name.clone();

                            // Get TLEs from cache (set by coordinator) or fetch if
                            // this group's coordinator ran on a different pod.
                            let tles = {
                                let cached = tle_cache
                                    .read()
                                    .await
                                    .get(&group)
                                    .map(|(_, t)| t.clone());

                                if let Some(t) = cached {
                                    t
                                } else {
                                    log::info!(
                                        "Worker: fetching TLEs for group={group} (coordinator on another pod)"
                                    );
                                    match fetch_tles(&client, &group).await {
                                        Some(t) => {
                                            tle_cache.write().await.insert(
                                                group.clone(),
                                                (std::time::Instant::now(), t.clone()),
                                            );
                                            t
                                        }
                                        None => {
                                            log::warn!(
                                                "Worker: TLE fetch failed for group={group}, failing chunk"
                                            );
                                            let _ = crate::db::fail_chunk(
                                                &pool,
                                                chunk.chunk_id,
                                                chunk.screening_id,
                                                "TLE fetch failed",
                                            )
                                            .await;
                                            continue;
                                        }
                                    }
                                }
                            };

                            screen_chunk(Some(pool.clone()), chunk, &tles).await;
                        }
                        Ok(None) => {
                            consecutive_idle += 1;
                            if consecutive_idle >= 10 {
                                log::debug!("Worker: no chunks for 10s, exiting");
                                break;
                            }
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                        Err(e) => {
                            log::warn!("Worker: claim_next_chunk error: {e}");
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                    }
                }
            });
        }
    }

    log::info!("Starting Server on {}", addr);
    HttpServer::new(move || {
        let routes = generate_route_list(App);

        // Rate limiter for authentication endpoints: 5 requests per minute
        let auth_rate_limiter = RateLimiter::new(5, Duration::from_secs(60));

        let visitor_logger = VisitorLogger::new(pool.clone(), http_client.clone());

        let mut app = actix_web::App::new()
            .route(
                "/api/lighthouse",
                web::post()
                    .to(upload_lighthouse_report)
                    .wrap(auth_rate_limiter),
            )
            .route("/api/metrics/stream", web::get().to(metrics_stream))
            .route("/world-map.svg", web::get().to(world_map))
            .route("/api/{tail:.*}", leptos_actix::handle_server_fns())
            .route("/health_check", web::get().to(health_check))
            .route("/robots.txt", web::get().to(robots_txt))
            .leptos_routes(routes, {
                let leptos_options = conf.leptos_options.clone();
                move || {
                    view! {
                        <!DOCTYPE html>
                        <html lang="en">
                            <head>
                                <meta charset="utf-8" />
                                <meta
                                    name="viewport"
                                    content="width=device-width, initial-scale=1"
                                />
                                <HydrationScripts options=leptos_options.clone() />
                                <MetaTags />
                            </head>
                            <body>
                                <App />
                            </body>
                        </html>
                    }
                }
            })
            .service(Files::new(
                "/jaydanhoward_wasm",
                wasm_dir.to_string_lossy().as_ref(),
            ))
            .service(Files::new("/", assets_root.to_string_lossy().as_ref()))
            .wrap(visitor_logger)
            .wrap(CacheControl)
            .wrap(SecurityHeaders)
            .wrap(actix_web::middleware::Compress::default());

        if let Some(ref p) = pool {
            app = app.app_data(web::Data::new(p.clone()));
        }
        app = app.app_data(world_map_data.clone());
        app = app.app_data(tle_cache.clone());
        app = app.app_data(conjunction_cache.clone());

        app
    })
    .bind(&addr)?
    .run()
    .await
}
