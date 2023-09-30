use leptos::*;
use leptos_router::{use_location, use_route, Outlet};

#[component]
pub fn Skills() -> impl IntoView {
    let location = use_location();
    let route = use_route();
    let pathname = move || location.pathname.get();

    let routes = vec![
        ("great", "am great at"),
        ("better", "am getting better at"),
        ("interested", "am interested in"),
    ];

    view! {
            <div>
                <div class="flex flex-row gap-10 mb-20">
                    <span>"Things I"</span>
                    {routes.into_iter()
                        .map(|(r, display_text)| {
                            let is_match = move || pathname().contains(r);
                            let is_not_match = move || !is_match();

                            view! {
                                <a
                                    href={route.resolve_path(r)}
                                    class=("underline", is_match)
                                    class=("font-bold", is_match)
                                    class=("cursor-default", is_match)
                                    class=("cursor-pointer", is_not_match)
                                    class=("hover:underline", is_not_match)
                                    class=("no-underline", is_not_match)
                                    class=("font-medium", is_not_match)
                                >
                                    {display_text}
                                </a>
                            }
                        })
                        .collect_view()}
                </div>
                <Outlet/>
        </div>
    }
}

#[component]
pub fn GreatAt() -> impl IntoView {
    view! {
        <ul class="list-disc list-outside space-y-4">
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
pub fn BetterAt() -> impl IntoView {
    view! {
        <ul class="list-disc list-outside space-y-4">
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
        <ul class="list-disc list-outside space-y-4">
            <li>Flight sims</li>
            <li>"Motorcyling, Onewheeling, transportation with <= 3 wheels"</li>
            <li>Space - KSP</li>
            <li>WASM</li>
            <li>GoLang</li>
            <li>Aerospace engineering</li>
        </ul>
    }
}
