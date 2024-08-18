#[cfg(feature = "ssr")]
use {
    crate::components::App,
    crate::prometheus_client::{query_prometheus, PrometheusData, PrometheusResult},
    crate::routes::{health_check, robots_txt, upload_lighthouse_report},
    crate::telemtry::{get_subscriber, init_subscriber},
    actix_files::Files,
    actix_web::{web, HttpServer, Responder},
    leptos::*,
    leptos_actix::{generate_route_list, LeptosRoutes},
    pulldown_cmark::{html, Options, Parser},
    std::env,
    std::fs::read_to_string,
};

#[cfg(feature = "ssr")]
async fn convert_resume_md_to_html() -> String {
    let site_dir = env::var("LEPTOS_SITE_ROOT").unwrap();

    match read_to_string(format!("{}/resume.md", site_dir)) {
        Ok(markdown_content) => {
            let options = Options::empty();
            let parser = Parser::new_ext(&markdown_content, options);
            let mut html_output = String::new();
            html::push_html(&mut html_output, parser);

            html_output
        }
        Err(_) => "<div />".into(),
    }
}

#[cfg(feature = "ssr")]
async fn get_metrics() -> impl Responder {
    let query = r#"sum(rate(container_cpu_usage_seconds_total[5m])) by (cluster)"#;

    match query_prometheus(query).await {
        Ok(data) => web::Json(data),
        Err(e) => {
            println!("Error querying Prometheus: {:?}", e);
            web::Json(PrometheusData {
                status: "error".to_string(),
                data: PrometheusResult {
                    result_type: "none".to_string(),
                    result: vec![],
                },
            })
        }
    }
}

#[cfg(feature = "ssr")]
pub async fn run() -> Result<(), std::io::Error> {
    let subscriber = get_subscriber("jaydanhoward".into(), "debug".into(), std::io::stdout);
    init_subscriber(subscriber);
    let conf = get_configuration(None).await.unwrap();
    let addr = conf.leptos_options.site_addr;

    let resume = convert_resume_md_to_html().await;
    let metrics = get_metrics().await;
    let routes = generate_route_list(|| view! { <App /> });

    let server = HttpServer::new(move || {
        let leptos_options = &conf.leptos_options;
        let site_root = &leptos_options.site_root;

        actix_web::App::new()
            .route("/api/lighthouse", web::post().to(upload_lighthouse_report))
            .route("/health_check", web::get().to(health_check))
            .route("/robots.txt", web::get().to(robots_txt))
            .service(Files::new("/pkg", format!("{site_root}/pkg")))
            .service(Files::new("/assets", site_root))
            .leptos_routes(
                leptos_options.to_owned(),
                routes.to_owned(),
                || view! { <App /> },
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
