/// Replacement for `HydrationScripts` that appends `?v={hash}` to both the JS and WASM URLs.
/// This lets Cloudflare (and browsers) cache the pair with `immutable` headers while
/// guaranteeing that a new deploy produces new URLs, preventing stale-pair LinkErrors.
#[cfg(feature = "ssr")]
#[leptos::component]
fn WasmScripts(options: leptos::prelude::LeptosOptions, version: std::sync::Arc<String>) -> impl leptos::IntoView {
    use leptos::prelude::*;

    let pkg = options.site_pkg_dir.to_string();
    let name = options.output_name.to_string();
    let v = (*version).clone();

    let js_url   = format!("/{pkg}/{name}.js?v={v}");
    let wasm_url = format!("/{pkg}/{name}_bg.wasm?v={v}");

    // Inline the hydration bootstrap directly so we can control both URLs.
    let script = format!(
        "(function(){{import(\"{js_url}\").then(function(m){{m.default({{module_or_path:\"{wasm_url}\"}}).then(function(){{m.hydrate();}});}});}})();"
    );

    view! {
        <link rel="modulepreload" href=js_url crossorigin="" />
        <link
            rel="preload"
            href=wasm_url
            r#as="fetch"
            r#type="application/wasm"
            crossorigin=""
        />
        <script type="module">{script}</script>
    }
}

#[cfg(feature = "ssr")]
pub async fn run() -> Result<(), std::io::Error> {
    use crate::components::conjunction::ConjunctionCache;
    use crate::components::satellite_tracker::TleCache;
    use crate::components::App;
    use crate::network_spike::NetworkSpikeDetector;
    use crate::db::create_pool;
    use crate::middleware::cache_control::cache_control;
    use crate::middleware::rate_limit::RateLimiter;
    use crate::middleware::security_headers::security_headers;
    use crate::middleware::visitor_logger::visitor_logger_fn;
    use crate::routes::{
        fetch_world_map_svg, health_check, ingest_claude_audit, metrics_stream, robots_txt,
        upload_lighthouse_report, upload_security_audit, world_map, WorldMapSvg,
    };
    use crate::telemtry::{get_subscriber, init_subscriber};
    use axum::{
        extract::Extension,
        middleware,
        routing::{get, post},
        Router,
    };
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use leptos_meta::MetaTags;
    use runfiles::{rlocation, Runfiles};
    use std::sync::Arc;
    use std::time::Duration;
    use tower_http::{compression::CompressionLayer, services::ServeDir};
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
                log::warn!("Failed to connect to Postgres: {e}");
                None
            }
        },
        Err(_) => {
            log::info!("DATABASE_URL not set, visitor tracking disabled");
            None
        }
    };

    let pool_arc: Option<Arc<sqlx::PgPool>> = pool.clone().map(Arc::new);

    let http_client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .build()
        .expect("Failed to build HTTP client");

    log::info!("Fetching world map from Natural Earth...");
    let world_map_svg = fetch_world_map_svg(&http_client).await;
    log::info!("World map ready ({} bytes)", world_map_svg.len());

    let world_map_data = Arc::new(WorldMapSvg(world_map_svg));
    let tle_cache = Arc::new(TleCache::new(std::collections::HashMap::new()));
    let conjunction_cache = Arc::new(ConjunctionCache::new(std::collections::HashMap::new()));

    // Seed in-memory TLE cache from DB so first requests don't cold-fetch CelesTrak.
    if let Some(ref p) = pool {
        match crate::db::load_all_tle_groups(p).await {
            Ok(groups) => {
                let mut w = tle_cache.write().await;
                for (group, tles) in &groups {
                    log::info!("TLE cache: restored {} satellites for group={group} from DB", tles.len());
                    w.insert(group.clone(), (std::time::Instant::now(), tles.clone()));
                }
            }
            Err(e) => log::warn!("TLE cache: failed to load from DB: {e}"),
        }
    }

    // Load persisted thresholds from DB, fall back to defaults.
    let (spike_multiplier, spike_floor_mbps) = if let Some(ref p) = pool {
        crate::db::load_spike_config(p).await
    } else {
        (1.5, 100.0)
    };
    log::info!("Spike detector: multiplier={spike_multiplier:.2} floor={spike_floor_mbps:.1} Mbps");
    // At 1s SSE tick: 180 samples = 3-min window, 30 samples = 30s warmup.
    let spike_detector = Arc::new(tokio::sync::Mutex::new(
        NetworkSpikeDetector::new(spike_multiplier, spike_floor_mbps, 180, 30),
    ));

    // Background spike detection loop — skipped when Prometheus is not configured
    if std::env::var("PROMETHEUS_URL").is_ok() {
        let detector = spike_detector.clone();
        let pool_opt = pool_arc.clone();

        tokio::spawn(async move {
            let mut heartbeat_tick: u64 = 0;
            let mut consecutive_failures: u32 = 0;
            loop {
                let tx_mbps = match crate::prometheus_client::query_prometheus(
                    "sum(rate(node_network_transmit_bytes_total{\
                      device!~\"lo|veth.*|docker.*|br-.*|cni.*|tunl.*|cilium.*|lxc.*|flannel.*|dummy.*\"\
                    }[2m])) * 8 / 1000000",
                )
                .await
                {
                    Ok(data) => {
                        consecutive_failures = 0;
                        data.data
                            .result
                            .first()
                            .and_then(|m| m.value.1.parse::<f64>().ok())
                            .unwrap_or(0.0)
                    }
                    Err(e) => {
                        consecutive_failures += 1;
                        // Exponential backoff: 2s, 4s, 8s, … capped at 5 minutes
                        let backoff = Duration::from_secs(
                            2_u64.pow(consecutive_failures.min(8)).min(300),
                        );
                        log::warn!(
                            "Spike detector: Prometheus query failed \
                             (attempt {consecutive_failures}, retry in {backoff:?}): {e}"
                        );
                        tokio::time::sleep(backoff).await;
                        continue;
                    }
                };
                heartbeat_tick += 1;

                if heartbeat_tick % 300 == 1 {
                    let (mult, floor) = {
                        let det = detector.lock().await;
                        (det.multiplier, det.floor_mbps)
                    };
                    log::info!("Spike detector heartbeat: tx={tx_mbps:.1} Mbps  threshold={mult:.1}x  floor={floor:.0} Mbps");
                }

                let spike = detector.lock().await.check(tx_mbps);
                if let Some((spike_mbps, baseline_mbps)) = spike {
                    let claimed = if let Some(ref p) = pool_opt {
                        crate::db::try_claim_spike(p).await
                    } else {
                        true
                    };

                    if !claimed {
                        log::debug!(
                            "Spike detector: {spike_mbps:.1} Mbps spike already claimed this bucket"
                        );
                    }

                    if claimed {
                        log::info!("Network spike detected: {spike_mbps:.1} Mbps (baseline {baseline_mbps:.1} Mbps) — calling Claude");
                        let pool_ref = pool_opt.clone();
                        let detector_ref = detector.clone();
                        tokio::spawn(async move {
                            match crate::network_spike::explain_spike(
                                spike_mbps,
                                baseline_mbps,
                                pool_ref.as_deref(),
                            )
                            .await
                            {
                                Ok((pods, explanation, significance)) => {
                                    log::info!(
                                        "Spike explained (significance={significance}/10): {explanation}"
                                    );
                                    let (new_mult, new_floor) = {
                                        let mut det = detector_ref.lock().await;
                                        det.apply_feedback(significance);
                                        (det.multiplier, det.floor_mbps)
                                    };
                                    if let Some(ref p) = pool_ref {
                                        let top_pods_json = serde_json::to_value(
                                            pods.iter()
                                                .map(|pod| serde_json::json!({
                                                    "namespace": pod.namespace,
                                                    "pod": pod.pod,
                                                    "mbps": pod.mbps,
                                                }))
                                                .collect::<Vec<_>>(),
                                        )
                                        .unwrap_or_default();
                                        let _ = crate::db::save_spike_config(p, new_mult, new_floor).await;
                                        let _ = crate::db::insert_network_insight(
                                            p,
                                            spike_mbps,
                                            baseline_mbps,
                                            &top_pods_json,
                                            &explanation,
                                        )
                                        .await;
                                    }
                                }
                                Err(e) => log::warn!("Failed to explain spike: {e}"),
                            }
                        });
                    }
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
    } // end PROMETHEUS_URL check

    // Pre-warm TLE cache at startup and refresh every 6 hours, regardless of DB availability.
    // Without this, the first browser request cold-fetches from CelesTrak (up to 30s hang).
    {
        use crate::components::satellite_tracker::TleData;

        const WARM_GROUPS: [&str; 5] = ["active", "starlink", "stations", "gps-ops", "visual"];
        const TLE_REFRESH_SECS: u64 = 6 * 60 * 60;

        fn parse_tles_warmup(text: &str) -> Vec<TleData> {
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

        // Spread pod startup across up to 60s so replicas don't burst CelesTrak simultaneously.
        let jitter_secs = rand::random::<u64>() % 60;

        for group in WARM_GROUPS {
            let cache = tle_cache.clone();
            let client = http_client.clone();
            let pool_opt = pool_arc.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(jitter_secs)).await;

                loop {
                    // Skip CelesTrak if DB already has fresh data — lets one pod do the
                    // fetch while others see fresh DB data on their next iteration.
                    let db_fresh = match pool_opt {
                        Some(ref p) => matches!(
                            crate::db::tle_group_age(p, group).await,
                            Ok(Some(t)) if chrono::Utc::now().signed_duration_since(t) < chrono::Duration::hours(6)
                        ),
                        None => false,
                    };

                    if db_fresh {
                        log::info!("TLE warm-up: DB fresh for group={group}, skipping fetch");
                    } else {
                        let url = format!(
                            "https://celestrak.org/NORAD/elements/gp.php?GROUP={group}&FORMAT=tle"
                        );
                        match client
                            .get(&url)
                            .timeout(Duration::from_secs(60))
                            .send()
                            .await
                        {
                            Ok(resp) if resp.status().is_success() => {
                                if let Ok(text) = resp.text().await {
                                    let tles = parse_tles_warmup(&text);
                                    if !tles.is_empty() {
                                        log::info!(
                                            "TLE cache warm-up: {} satellites for group={group}",
                                            tles.len()
                                        );
                                        cache.write().await.insert(
                                            group.to_string(),
                                            (std::time::Instant::now(), tles.clone()),
                                        );
                                        if let Some(ref p) = pool_opt {
                                            if let Err(e) = crate::db::save_tle_group(p, group, &tles).await {
                                                log::warn!("TLE warm-up: failed to persist group={group}: {e}");
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(resp) => {
                                log::warn!("TLE warm-up: HTTP {} for group={group}", resp.status());
                            }
                            Err(e) => {
                                log::warn!("TLE warm-up: fetch failed for group={group}: {e}");
                            }
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(TLE_REFRESH_SECS)).await;
                }
            });
        }
    }

    // Startup conjunction screening — chunk-based distributed approach.
    {
        use crate::components::satellite_tracker::TleData;

        const CHUNK_SIZE: usize = 50;
        const GROUPS: [&str; 5] = ["stations", "gps-ops", "visual", "active", "starlink"];

        let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());

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

        const SCREENING_INTERVAL_SECS: u64 = 60 * 60;

        // No-DB path: run in-memory screenings once so the cache is populated in dev/CI.
        if pool_arc.is_none() {
            for group in GROUPS {
                let cache = conjunction_cache.clone();
                let tle_cache = tle_cache.clone();
                let client = http_client.clone();
                tokio::spawn(async move {
                    if let Some(tles) = fetch_tles(&client, group).await {
                        tle_cache
                            .write()
                            .await
                            .insert(group.to_string(), (std::time::Instant::now(), tles.clone()));
                        crate::components::conjunction::screen_and_store(
                            None,
                            Some(cache),
                            group,
                            &tles,
                            None,
                        )
                        .await;
                    }
                });
            }
        }

        for group in GROUPS {
            let pool_opt  = pool_arc.clone();
            let tle_cache = tle_cache.clone();
            let client    = http_client.clone();
            let hostname  = hostname.clone();

            tokio::spawn(async move {
                let pool = match pool_opt {
                    None => return,
                    Some(p) => p,
                };

                loop {
                    let screening_id = match crate::db::try_claim_conjunction_startup(
                        &pool, group, &hostname, 60,
                    ).await {
                        Ok(Some(id)) => id,
                        Ok(None) => {
                            log::debug!("Coordinator: group={group} already claimed this cycle");
                            tokio::time::sleep(Duration::from_secs(SCREENING_INTERVAL_SECS)).await;
                            continue;
                        }
                        Err(e) => {
                            log::warn!("Coordinator: DB claim failed for group={group}: {e}");
                            tokio::time::sleep(Duration::from_secs(30)).await;
                            continue;
                        }
                    };

                    log::info!("Coordinator: fetching TLEs for group={group}");
                    let tles = match fetch_tles(&client, group).await {
                        Some(t) => t,
                        None => {
                            log::warn!("Coordinator: TLE fetch failed for group={group}");
                            tokio::time::sleep(Duration::from_secs(SCREENING_INTERVAL_SECS)).await;
                            continue;
                        }
                    };

                    let n = tles.len();
                    let total_pairs = n * n.saturating_sub(1) / 2;
                    log::info!("Coordinator: {n} TLEs ({total_pairs} pairs) for group={group}");

                    if let Err(e) =
                        crate::db::update_conjunction_total_pairs(&pool, screening_id, total_pairs as i64)
                            .await
                    {
                        log::warn!("Coordinator: total_pairs update failed for {group}: {e}");
                    }

                    match crate::db::create_chunks(&pool, screening_id, group, n, CHUNK_SIZE).await {
                        Ok(k) => log::info!("Coordinator: created {k} chunks for group={group}"),
                        Err(e) => {
                            log::error!("Coordinator: create_chunks failed for {group}: {e}");
                            tokio::time::sleep(Duration::from_secs(SCREENING_INTERVAL_SECS)).await;
                            continue;
                        }
                    }

                    tle_cache
                        .write()
                        .await
                        .insert(group.to_string(), (std::time::Instant::now(), tles));

                    tokio::time::sleep(Duration::from_secs(SCREENING_INTERVAL_SECS)).await;
                }
            });
        }

        // Worker loop
        {
            let pool_opt  = pool_arc.clone();
            let tle_cache = tle_cache.clone();
            let client    = http_client.clone();
            let hostname  = hostname.clone();

            tokio::spawn(async move {
                use crate::components::conjunction::screen_chunk;

                let pool = match pool_opt {
                    None    => return,
                    Some(p) => p,
                };

                tokio::time::sleep(Duration::from_millis(500)).await;

                loop {
                    match crate::db::claim_next_chunk(&pool, &hostname).await {
                        Ok(Some(chunk)) => {
                            let group = chunk.group_name.clone();

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
                            tokio::time::sleep(Duration::from_secs(15)).await;
                        }
                        Err(e) => {
                            log::warn!("Worker: claim_next_chunk error: {e}");
                            tokio::time::sleep(Duration::from_secs(30)).await;
                        }
                    }
                }
            });
        }
    }

    // Compute a content hash of the WASM binary so the HTML shell can embed ?v={hash}
    // in the JS/WASM URLs, enabling immutable caching for those assets.
    let wasm_version: Arc<String> = {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let wasm_path = wasm_dir.join("jaydanhoward_wasm_bg.wasm");
        let bytes = std::fs::read(&wasm_path).unwrap_or_default();
        let mut h = DefaultHasher::new();
        bytes.hash(&mut h);
        Arc::new(format!("{:016x}", h.finish()))
    };
    log::info!("WASM content version: {wasm_version}");

    let leptos_options = conf.leptos_options.clone();
    let routes = generate_route_list(App);

    // Rate limiter for lighthouse endpoint: 5 requests per minute
    let auth_rate_limiter = RateLimiter::new(5, Duration::from_secs(60));

    let pool_for_mw = pool.clone();
    let http_client_for_mw = http_client.clone();

    log::info!("Starting Server on {addr}");

    let app = Router::new()
        .route(
            "/api/lighthouse",
            post(upload_lighthouse_report).layer(middleware::from_fn(move |req, next| {
                let limiter = auth_rate_limiter.clone();
                async move { limiter.check_middleware(req, next).await }
            })),
        )
        .route("/api/audit/claude", post(ingest_claude_audit))
        .route("/api/security-audit", post(upload_security_audit))
        .route("/api/metrics/stream", get(metrics_stream))
        .route("/world-map.svg", get(world_map))
        .route(
            "/api/{*fn_name}",
            post(leptos_axum::handle_server_fns).get(leptos_axum::handle_server_fns),
        )
        .route("/health_check", get(health_check))
        .route("/robots.txt", get(robots_txt))
        .leptos_routes(
            &leptos_options,
            routes,
            {
                let leptos_options = leptos_options.clone();
                let wasm_version = wasm_version.clone();
                move || {
                    let ver = wasm_version.clone();
                    view! {
                        <!DOCTYPE html>
                        <html lang="en">
                            <head>
                                <meta charset="utf-8" />
                                <meta
                                    name="viewport"
                                    content="width=device-width, initial-scale=1"
                                />
                                <WasmScripts options=leptos_options.clone() version=ver />
                                <MetaTags />
                            </head>
                            <body>
                                <App />
                            </body>
                        </html>
                    }
                }
            },
        )
        .nest_service(
            "/jaydanhoward_wasm",
            ServeDir::new(wasm_dir.to_string_lossy().as_ref()),
        )
        .fallback_service(ServeDir::new(assets_root.to_string_lossy().as_ref()))
        // Global middleware (applied outermost = last listed)
        .layer(middleware::from_fn(move |req, next| {
            let pool = pool_for_mw.clone();
            let http_client = http_client_for_mw.clone();
            async move { visitor_logger_fn(pool, http_client, req, next).await }
        }))
        .layer(middleware::from_fn(cache_control))
        .layer(middleware::from_fn(security_headers))
        .layer(CompressionLayer::new())
        // State extensions
        .layer(Extension(pool_arc.clone()))
        .layer(Extension(world_map_data))
        .layer(Extension(tle_cache))
        .layer(Extension(conjunction_cache));

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app.with_state(leptos_options).into_make_service()).await
}
