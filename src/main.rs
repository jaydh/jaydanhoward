#![recursion_limit = "256"]

mod components;
#[cfg(feature = "ssr")]
mod middleware;
mod prometheus_client;
mod routes;
mod startup;
mod telemtry;

#[cfg(feature = "ssr")]
#[actix_web::main]
async fn main() -> () {
    use startup::run;

    let _ = run().await;
}

#[cfg(not(any(feature = "ssr")))]
pub fn main() {}
