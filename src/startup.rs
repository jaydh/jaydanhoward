#[cfg(feature = "ssr")]
pub async fn run() -> Result<(), std::io::Error> {
    use crate::components::App;
    use crate::middleware::cache_control::CacheControl;
    use crate::middleware::rate_limit::RateLimiter;
    use crate::middleware::security_headers::SecurityHeaders;
    use crate::routes::{health_check, metrics_stream, robots_txt, upload_lighthouse_report};
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
    // In `bazel run` mode, rlocation! always uses manifest mode (because a MANIFEST file exists
    // in the runfiles tree). Manifest mode resolves source files to the source directory and
    // WASM outputs to the Bazel cache — no common parent, so leptos_toml_path.parent() misses WASM.
    // The runfiles symlink tree at `<binary>.runfiles/_main/` has everything linked correctly.
    // Derive it from env vars in priority order:
    // 1. RUNFILES_DIR (set by `bazel run` / `bazel test`)
    // 2. Derive from RUNFILES_MANIFEST_FILE: "foo.runfiles_manifest" → "foo.runfiles/"
    // 3. Fall back to leptos_toml_path.parent() for OCI containers (directory mode runfiles)
    let main_path = std::env::var("RUNFILES_DIR")
        .map(|d| std::path::PathBuf::from(d).join("_main"))
        .or_else(|_| {
            std::env::var("RUNFILES_MANIFEST_FILE").map(|manifest_file| {
                let manifest_path = std::path::PathBuf::from(&manifest_file);
                let manifest_name = manifest_path
                    .file_name()
                    .expect("manifest has filename")
                    .to_string_lossy()
                    .into_owned();
                let runfiles_name = manifest_name
                    .strip_suffix("_manifest")
                    .unwrap_or(&manifest_name)
                    .to_string();
                manifest_path
                    .parent()
                    .expect("manifest has parent")
                    .join(runfiles_name)
                    .join("_main")
            })
        })
        .unwrap_or_else(|_| {
            leptos_toml_path
                .parent()
                .expect("Failed to locate main")
                .to_path_buf()
        });

    let conf = get_configuration(Some(leptos_toml_path.to_string_lossy().as_ref()))
        .expect("Failed to read conf");

    let addr = conf.leptos_options.site_addr;

    log::info!("Starting Server on {}", addr);
    HttpServer::new(move || {
        let routes = generate_route_list(App);

        // Rate limiter for authentication endpoints: 5 requests per minute
        let auth_rate_limiter = RateLimiter::new(5, Duration::from_secs(60));

        actix_web::App::new()
            .route(
                "/api/lighthouse",
                web::post()
                    .to(upload_lighthouse_report)
                    .wrap(auth_rate_limiter),
            )
            .route("/api/metrics/stream", web::get().to(metrics_stream))
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
            .service(Files::new("/", main_path.to_string_lossy().as_ref()))
            .wrap(CacheControl)
            .wrap(SecurityHeaders)
            .wrap(actix_web::middleware::Compress::default())
    })
    .bind(&addr)?
    .run()
    .await
}
