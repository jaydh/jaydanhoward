use crate::components::link::Link;
use leptos::*;
use leptos_router::Outlet;

#[component]
pub fn Skills() -> impl IntoView {
    let routes = vec![
        ("experienced", "Experienced In"),
        ("proficient", "Proficient In"),
        ("interested", "Interested In"),
    ];

    view! {
        <div>
            <div class="flex flex-row gap-10 mb-20">
                {routes
                    .into_iter()
                    .map(|(path, display_text)| {
                        view! { <Link path=path display_text=display_text/> }
                    })
                    .collect_view()}
            </div>
            <Outlet/>
        </div>
    }
}

#[component]
pub fn Experienced() -> impl IntoView {
    view! {
        <ul class="flex flex-col list-none space-y-4 items-center">
            <li>Fullstack web development</li>
            <li>Typescript/Javascript/ES6</li>
            <li>React</li>
            <li>SQL (MySQL, Postgres)</li>
            <li>Designing performant and fault-tolerant RESTful APIs</li>
            <li>Service telemetry and root cause analysis</li>
            <li>Leading teams</li>
            <li>Talking to stake holders</li>
            <li>Finding high value wins (especially the ones with low costs)</li>
            <li>Mentoring junior engineers</li>
            <li>Getting it done</li>
        </ul>
    }
}

#[component]
pub fn Proficient() -> impl IntoView {
    view! {
        <ul class="flex flex-col list-none space-y-4 items-center">
            <li>Rust</li>
            <li>Leptos</li>
            <li>Kubernetes</li>
            <li>Motorcyling</li>
            <li>Dog parenting</li>
            <li>Camping</li>
        </ul>
    }
}

#[component]
pub fn InterestedIn() -> impl IntoView {
    view! {
        <ul class="flex flex-col list-none space-y-4 items-center">
            <li>Flight sims</li>
            <li>"Motorcyling, Onewheeling, transportation with <= 3 wheels"</li>
            <li>Space - KSP</li>
            <li>WASM</li>
            <li>GoLang</li>
            <li>Aerospace engineering</li>
        </ul>
    }
}
