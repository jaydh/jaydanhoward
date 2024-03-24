#![allow(clippy::all)]

use crate::components::source_anchor::SourceAnchor;
use leptos::*;

#[server(ActixExtract, "/api")]
pub async fn actix_extract() -> Result<String, ServerFnError> {
    use actix_web::web::Data;
    use leptos_actix::extract;
    let resume: Data<String> = extract().await?;
    Ok(resume.to_string())
}

#[component]
pub fn Resume() -> impl IntoView {
    let once = create_resource(|| (), |_| async move { actix_extract().await });

    view! {
        <SourceAnchor href="#[git]"/>
        <Suspense>
            {move || {
                once.get()
                    .map(|resume_html| {
                        view! { <div class="resume" inner_html=resume_html.ok()></div> }
                    })
            }}

        </Suspense>
    }
}
