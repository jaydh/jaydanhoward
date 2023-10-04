use leptos::*;
use leptos_meta::Meta;
use leptos_router::ActionForm;

#[server(ToggleDarkMode, "/api")]
pub async fn toggle_dark_mode(prefers_dark: bool) -> Result<bool, ServerFnError> {
    use actix_web::http::header::{HeaderMap, HeaderValue, SET_COOKIE};
    use leptos_actix::{ResponseOptions, ResponseParts};

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

    std::thread::sleep(std::time::Duration::from_millis(250));

    response.overwrite(response_parts);
    Ok(prefers_dark)
}

#[cfg(not(feature = "ssr"))]
pub fn initial_prefers_dark() -> bool {
    use wasm_bindgen::JsCast;

    let doc = document().unchecked_into::<web_sys::HtmlDocument>();
    let cookie = doc.cookie().unwrap_or_default();
    match cookie.contains("darkmode=") {
        true => cookie.contains("darkmode=true"),
        false => get_client_prefers_dark(),
    }
}

#[cfg(not(feature = "ssr"))]
pub fn get_client_prefers_dark() -> bool {
    let w = window();
    match w.match_media("(prefers-color-scheme: dark)") {
        Ok(o) => match o {
            Some(media_query_list) => media_query_list.matches(),
            None => false,
        },
        Err(..) => false,
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
pub fn DarkModeToggle(set_dark_mode_enabled: WriteSignal<bool>) -> impl IntoView {
    let initial = initial_prefers_dark();

    let toggle_dark_mode_action = create_server_action::<ToggleDarkMode>();
    let input = toggle_dark_mode_action.input();
    let value = toggle_dark_mode_action.value();

    let prefers_dark = move || match (input(), value()) {
        (Some(submission), _) => submission.prefers_dark,
        (_, Some(Ok(value))) => value,
        _ => initial,
    };

    create_effect(move |_| {
        set_dark_mode_enabled(prefers_dark());
    });

    let color_scheme = move || {
        if prefers_dark() {
            "dark".to_string()
        } else {
            "light".to_string()
        }
    };

    view! {
        <Meta
            name="color-scheme"
            content=color_scheme
        />
        <ActionForm action=toggle_dark_mode_action>
            <input
                type="hidden"
                name="prefers_dark"
                value=move || (!prefers_dark()).to_string()
            />
            <button
                type="submit"
                class="fixed bottom-12 right-20"
            >
                <i class="far fa-lightbulb"/>
            </button>
        </ActionForm>
    }
}
