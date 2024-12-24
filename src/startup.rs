#[cfg(feature = "ssr")]
pub async fn run() -> Result<(), std::io::Error> {
    use crate::components::App;
    use crate::routes::{health_check, robots_txt, upload_lighthouse_report};
    use crate::telemtry::{get_subscriber, init_subscriber};
    use actix_files::Files;
    use actix_web::{web, HttpServer};
    use leptos::prelude::*;
    use leptos_actix::{generate_route_list, LeptosRoutes};
    use runfiles::{rlocation, Runfiles};
    use tracing::log;

    let subscriber = get_subscriber("jaydanhoward".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    console_error_panic_hook::set_once();

    let r = Runfiles::create().expect("Must run using bazel with runfiles");
    let leptos_toml_path = rlocation!(r, "_main/leptos.toml").expect("Failed to locate runfile");
    let assets_path = rlocation!(r, "_main/assets").expect("Failed to locate assets");
    let pkg_path = rlocation!(r, "_main/pkg").expect("Failed to locate assets");

    let conf = get_configuration(Some(&leptos_toml_path.to_string_lossy().to_string()))
        .expect("Failed to read conf");

    let addr = conf.leptos_options.site_addr;

    log::info!("Starting Server on {}", addr);

    let server = HttpServer::new(move || {
        let routes = generate_route_list(App);
        let leptos_options = &conf.leptos_options;
        let site_root = &leptos_options.site_root;

        actix_web::App::new()
            .route("/api/lighthouse", web::post().to(upload_lighthouse_report))
            .route("/health_check", web::get().to(health_check))
            .route("/robots.txt", web::get().to(robots_txt))
            .service(Files::new(
                "/assets",
                assets_path.to_string_lossy().to_string(),
            ))
            .service(Files::new("/pkg", pkg_path.to_string_lossy().to_string()))
            .leptos_routes(routes, {
                let leptos_options = conf.leptos_options.clone();
                move || {
                    view! {
                        <!DOCTYPE html>
                        <html lang="en">
                            <head>
                                <meta charset="utf-8"/>
                                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                                <AutoReload options=leptos_options.clone() />
                                <HydrationScripts options=leptos_options.clone()/>
                            </head>
                            <body>
                                <App/>
                            </body>
                        </html>
                    }
                }
            })
            .wrap(actix_web::middleware::Compress::default())
    })
    .bind(&addr)?
    .run()
    .await;

    server
}
