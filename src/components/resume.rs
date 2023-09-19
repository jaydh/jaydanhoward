#![allow(clippy::all)]

use crate::components::source_anchor::SourceAnchor;
use leptos::*;

#[server(ActixExtract, "/api")]
pub async fn actix_extract() -> Result<String, ServerFnError> {
    use actix_web::web::Data;
    use leptos_actix::extract;

    extract(|resume: Data<String>| async move { resume.to_string() }).await
}

#[component]
pub fn Resume() -> impl IntoView {
    let once = create_resource(|| (), |_| async move { actix_extract().await });

    view! {
         <SourceAnchor href="#[git]" />
         <Suspense
            fallback=move || view! { <p>"Loading..."</p> }>
            {move || {
                once.get()
                    .map(|resume_html| view! { <div class="resume" inner_html=resume_html.ok() /> })
                }
            }
        </Suspense>
    }
}
