#[cfg(feature = "ssr")]
use {
    crate::components::App,
    crate::routes::{health_check, robots_txt, upload_lighthouse_report},
    crate::telemtry::{get_subscriber, init_subscriber},
    actix_files::Files,
    actix_web::{web, HttpServer},
    leptos::prelude::*,
    leptos_actix::{generate_route_list, LeptosRoutes},
    tracing::log,
};

#[cfg(feature = "ssr")]
pub async fn run() -> Result<(), std::io::Error> {
    let subscriber = get_subscriber("jaydanhoward".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    console_error_panic_hook::set_once();

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;

    let routes = generate_route_list(|| view! { <App /> });

    log::info!("Starting Server on {}", addr);
    let server = HttpServer::new(move || {
        let leptos_options = &conf.leptos_options;
        let site_root = &leptos_options.site_root;

        actix_web::App::new()
            .route("/api/lighthouse", web::post().to(upload_lighthouse_report))
            .route("/health_check", web::get().to(health_check))
            .route("/robots.txt", web::get().to(robots_txt))
            .service(Files::new("/pkg", format!("{site_root}/pkg")))
            .service(Files::new("/assets", site_root.as_ref()))
            .leptos_routes(routes.to_owned(), || view! { <App /> })
            .app_data(web::Data::new(leptos_options.to_owned()))
            .wrap(actix_web::middleware::Compress::default())
    })
    .bind(&addr)?
    .run()
    .await;

    server
}
