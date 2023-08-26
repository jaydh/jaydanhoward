#[cfg(feature = "ssr")]
use {
    crate::components::App,
    crate::routes::{health_check, upload_lighthouse_report},
    crate::telemtry::{get_subscriber, init_subscriber},
    actix_files::Files,
    actix_web::{web, HttpServer},
    leptos::*,
    leptos_actix::{generate_route_list, LeptosRoutes},
    pulldown_cmark::{html, Options, Parser},
    std::fs::read_to_string,
};

#[cfg(feature = "ssr")]
async fn convert_resume_md_to_html() -> String {
    let markdown_content = read_to_string("assets/resume.md").unwrap();
    let options = Options::empty();
    let parser = Parser::new_ext(&markdown_content, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    html_output
}

#[cfg(feature = "ssr")]
pub async fn run() -> Result<(), std::io::Error> {
    let subscriber = get_subscriber("jaydanhoward".into(), "debug".into(), std::io::stdout);
    init_subscriber(subscriber);
    let conf = get_configuration(None).await.unwrap();
    let addr = conf.leptos_options.site_addr;

    let resume = convert_resume_md_to_html().await;
    let routes = generate_route_list(|cx| view! { cx, <App/> });

    let server = HttpServer::new(move || {
        let leptos_options = &conf.leptos_options;
        let site_root = &leptos_options.site_root;

        actix_web::App::new()
            .route("/api/lighthouse", web::post().to(upload_lighthouse_report))
            .route("/health_check", web::get().to(health_check))
            .route("/api/{tail:.*}", leptos_actix::handle_server_fns())
            .service(Files::new("/pkg", format!("{site_root}/pkg")))
            .service(Files::new("/assets", site_root))
            .leptos_routes(
                leptos_options.to_owned(),
                routes.to_owned(),
                |cx| view! { cx, <App/> },
            )
            .app_data(web::Data::new(leptos_options.to_owned()))
            .app_data(web::Data::new(resume.to_owned()))
            .wrap(actix_web::middleware::Compress::default())
    })
    .bind(&addr)?
    .run()
    .await;

    server
}
