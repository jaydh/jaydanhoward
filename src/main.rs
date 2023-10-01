#[cfg(feature = "ssr")]
#[actix_web::main]
async fn main() -> () {
    use jaydanhoward::startup::run;

    let _ = run().await;
}

#[cfg(not(any(feature = "ssr", feature = "csr")))]
pub fn main() {}
