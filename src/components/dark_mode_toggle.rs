use leptos::prelude::*;
use leptos_meta::Meta;

#[server]
pub async fn toggle_dark_mode(prefers_dark: bool) -> Result<bool, ServerFnError> {
    use actix_web::http::header::{HeaderMap, HeaderValue, SET_COOKIE};
    use leptos_actix::{ResponseOptions, ResponseParts};
    dbg!("huh");

    let response =
        use_context::<ResponseOptions>().expect("to have leptos_actix::ResponseOptions provided");
    let mut response_parts = ResponseParts::default();
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&format!("darkmode={prefers_dark}; Path=/"))
            .expect("to create header value"),
    );
    response_parts.headers = headers;

    response.overwrite(response_parts);
    Ok(prefers_dark)
}

#[cfg(not(feature = "ssr"))]
pub fn get_client_prefers_dark() -> bool {
    let w = web_sys::window().expect("Failed to get window");
    match w.match_media("(prefers-color-scheme: dark)") {
        Ok(o) => match o {
            Some(media_query_list) => media_query_list.matches(),
            None => false,
        },
        Err(..) => false,
    }
}

#[cfg(not(feature = "ssr"))]
pub fn initial_prefers_dark() -> bool {
    use wasm_bindgen::JsCast;

    let doc = document().unchecked_into::<leptos::web_sys::HtmlDocument>();
    let cookie = doc.cookie().unwrap_or_default();
    match cookie.contains("darkmode=") {
        true => cookie.contains("darkmode=true"),
        false => get_client_prefers_dark(),
    }
}

#[cfg(feature = "ssr")]
pub fn initial_prefers_dark() -> bool {
    use_context::<actix_web::HttpRequest>()
        .and_then(|req| {
            req.cookies()
                .map(|cookies| {
                    cookies
                        .iter()
                        .any(|cookie| cookie.name() == "darkmode" && cookie.value() == "true")
                })
                .ok()
        })
        .unwrap_or(false)
}

#[component]
pub fn DarkModeToggle(
    dark_mode_enabled: ReadSignal<bool>,
    set_dark_mode_enabled: WriteSignal<bool>,
) -> impl IntoView {
    let initial = initial_prefers_dark();

    create_effect(move |_| {
        set_dark_mode_enabled(initial);
    });

    let color_scheme = move || {
        if dark_mode_enabled() {
            "dark".to_string()
        } else {
            "light".to_string()
        }
    };

    view! {
        <Meta name="color-scheme" content=color_scheme />
        <input type="hidden" name="prefers_dark" value=move || (!dark_mode_enabled()).to_string() />
        <button
            type="submit"
            class="fixed bottom-12 right-20"
            aria-label="light-dark-mode-toggle"
            on:click=move |_| {
                let dark_mode = dark_mode_enabled();
                leptos::task::spawn_local(async move {
                    let next = toggle_dark_mode(!dark_mode)
                        .await
                        .expect("Failed to update dark mode");
                    set_dark_mode_enabled(next);
                });
            }
        >
            <i class="far fa-lightbulb"></i>
        </button>
    }
}
