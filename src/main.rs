#[cfg(feature = "ssr")]
#[actix_web::main]
async fn main() -> () {
    use jaydanhoward::startup::main::run;
    use std::process::Command;
    use std::thread;
    use std::time::Duration;

    thread::spawn(|| {
        thread::sleep(Duration::from_secs(30));
        let output = Command::new("lighthouse")
            .args([
                "https://jaydanhoward-qwuri.ondigitalocean.app/",
                "--output-path",
                "./site/lighthouse.html",
                r#"--chrome-flags="--headless""#,
            ])
            .output()
            .expect("failed to execute process");
        dbg!(output);
    });

    run().await;
}

#[cfg(feature = "ssr")]
#[actix_web::get("favicon.ico")]
async fn favicon(
    leptos_options: actix_web::web::Data<leptos::LeptosOptions>,
) -> actix_web::Result<actix_files::NamedFile> {
    let leptos_options = leptos_options.into_inner();
    let site_root = &leptos_options.site_root;
    Ok(actix_files::NamedFile::open(format!(
        "{site_root}/favicon.ico"
    ))?)
}

#[cfg(not(any(feature = "ssr", feature = "csr")))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
    // see optional feature `csr` instead
}

#[cfg(all(not(feature = "ssr"), feature = "csr"))]
pub fn main() {
    // a client-side main function is required for using `trunk serve`
    // prefer using `cargo leptos serve` instead
    // to run: `trunk serve --open --features csr`
    use leptos::*;
    use leptos_start::app::*;
    use wasm_bindgen::prelude::wasm_bindgen;

    console_error_panic_hook::set_once();

    leptos::mount_to_body(move |cx| {
        // note: for testing it may be preferrable to replace this with a
        // more specific component, although leptos_router should still work
        view! { cx, <App/> }
    });
}
