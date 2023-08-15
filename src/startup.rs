#[cfg(feature = "ssr")]
pub mod main {
    use crate::app::App;
    use actix_files::Files;
    use actix_web::*;
    use actix_web::{web, HttpResponse, HttpServer};
    use leptos::*;
    use leptos_actix::{generate_route_list, LeptosRoutes};

    pub async fn health_check() -> HttpResponse {
        HttpResponse::Ok().finish()
    }

    pub async fn run() -> Result<(), std::io::Error> {
        let conf = get_configuration(None).await.unwrap();
        let addr = conf.leptos_options.site_addr;
        // Generate the list of routes in your Leptos App
        let routes = generate_route_list(|cx| view! { cx, <App/> });

        let server = HttpServer::new(move || {
            let leptos_options = &conf.leptos_options;
            let site_root = &leptos_options.site_root;

            App::new()
                .route("/api/{tail:.*}", leptos_actix::handle_server_fns())
                .service(Files::new("/pkg", format!("{site_root}/pkg")))
                .service(Files::new("/assets", site_root))
                .route("/health_check", web::get().to(health_check))
                .leptos_routes(
                    leptos_options.to_owned(),
                    routes.to_owned(),
                    |cx| view! { cx, <App/> },
                )
                .app_data(web::Data::new(leptos_options.to_owned()))
                .wrap(middleware::Compress::default())
        })
        .bind(&addr)?
        .run()
        .await;

        server
    }
}
