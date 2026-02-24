#[cfg(feature = "ssr")]
pub async fn run() -> Result<(), std::io::Error> {
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

    // Startup conjunction screening: fetch TLEs for all groups and screen in background.
    // Runs only when Postgres is available so results are ready before the first client connects.
    if let Some(startup_pool) = pool.clone() {
        use crate::components::conjunction::screen_and_store;
        use std::time::Duration;

        const GROUPS: &[&str] = &["stations", "gps-ops", "visual", "active", "starlink"];

        for &group in GROUPS {
            let pool_clone = startup_pool.clone();
            let group_str = group.to_string();
            let client_clone = http_client.clone();

            tokio::spawn(async move {
                // Small stagger to avoid thundering-herd on CelesTrak
                let idx = GROUPS.iter().position(|g| *g == group_str).unwrap_or(0);
                tokio::time::sleep(Duration::from_secs(idx as u64 * 3)).await;

                let url = format!(
                    "https://celestrak.org/NORAD/elements/gp.php?GROUP={group_str}&FORMAT=tle"
                );
                log::info!("Startup screening: fetching TLEs for group={}", group_str);

                let resp = match client_clone.get(&url).timeout(Duration::from_secs(60)).send().await {
                    Ok(r) if r.status().is_success() => r,
                    Ok(r) => {
                        log::warn!("Startup screening: CelesTrak returned {} for group={}", r.status(), group_str);
                        return;
                    }
                    Err(e) => {
                        log::warn!("Startup screening: TLE fetch failed for group={}: {}", group_str, e);
                        return;
                    }
                };

                let text = match resp.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        log::warn!("Startup screening: failed to read TLE body for group={}: {}", group_str, e);
                        return;
                    }
                };

                use crate::components::satellite_tracker::TleData;
                let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                let mut tles: Vec<TleData> = Vec::new();
                for chunk in lines.chunks(3) {
                    if chunk.len() == 3
                        && chunk[1].trim_start().starts_with("1 ")
                        && chunk[2].trim_start().starts_with("2 ")
                    {
                        tles.push(TleData {
                            name: chunk[0].trim().to_string(),
                            line1: chunk[1].trim().to_string(),
                            line2: chunk[2].trim().to_string(),
                        });
                    }
                }

                log::info!("Startup screening: {} TLEs for group={}", tles.len(), group_str);
                screen_and_store(&pool_clone, &group_str, &tles).await;
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

        app
    })
    .bind(&addr)?
    .run()
    .await
}
