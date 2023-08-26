#![allow(clippy::all)]

use leptos::*;

#[server(ActixExtract, "/api")]
pub async fn actix_extract(cx: Scope) -> Result<String, ServerFnError> {
    use actix_web::web::Data;
    use leptos_actix::extract;

    extract(cx, |resume: Data<String>| async move { resume.to_string() }).await
}

#[component]
pub fn Resume(cx: Scope) -> impl IntoView {
    let once = create_resource(
        cx,
        move || cx,
        |c: Scope| async move { actix_extract(c).await },
    );

    view! { cx,
         <Suspense
            fallback=move || view! { cx, <p>"Loading..."</p> }>
            {move || {
                once.read(cx)
                    .map(|resume_html| view! { cx, <div class="resume" inner_html=resume_html.ok() /> })
                }
            }
        </Suspense>
    }
}
