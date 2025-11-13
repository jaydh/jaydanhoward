#[cfg(feature = "ssr")]
pub async fn run() -> Result<(), std::io::Error> {
    use crate::components::App;
    use crate::middleware::cache_control::CacheControl;
    use crate::routes::{health_check, robots_txt, upload_lighthouse_report};
    use crate::telemtry::{get_subscriber, init_subscriber};
    use actix_files::Files;
    use actix_web::{web, HttpServer};
    use leptos::prelude::*;
    use leptos_actix::{generate_route_list, LeptosRoutes};
    use leptos_meta::MetaTags;
    use runfiles::{rlocation, Runfiles};
    use tracing::log;

    let subscriber = get_subscriber("jaydanhoward".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    console_error_panic_hook::set_once();

    let r = Runfiles::create().expect("Must run using bazel with runfiles");
    let leptos_toml_path = rlocation!(r, "_main/leptos.toml").expect("Failed to locate runfile");
    let main_path = rlocation!(r, "_main").expect("Failed to locate main");

    let conf = get_configuration(Some(leptos_toml_path.to_string_lossy().as_ref()))
        .expect("Failed to read conf");

    let addr = conf.leptos_options.site_addr;

    log::info!("Starting Server on {}", addr);
    HttpServer::new(move || {
        let routes = generate_route_list(App);

        actix_web::App::new()
            .route("/api/lighthouse", web::post().to(upload_lighthouse_report))
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
            .wrap(actix_web::middleware::Compress::default())
    })
    .bind(&addr)?
    .run()
    .await
}
