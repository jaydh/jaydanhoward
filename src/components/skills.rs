use leptos::*;
use leptos_router::{use_location, Outlet};

#[component]
pub fn Skills(cx: Scope) -> impl IntoView {
    let location = use_location(cx);
    let pathname = move || location.pathname.get();

    let routes = vec![
        ("great", "great at"),
        ("better", "getting better at"),
        ("interested", "interested in"),
    ];

    view! { cx,
            <div>
                <div class="flex flex-row gap-10 mb-20">
                    <span>"Things I'm"</span>
                    {routes.into_iter()
                        .map(|(route, display_text)| {
                            let is_match = move || pathname() == format!("/skills/{}", route);
                            let is_not_match = move || !is_match();

                            view! { cx,
                                <a
                                    href={route}
                                    class=("underline", is_match)
                                    class=("font-heavy", is_match)
                                    class=("cursor-default", is_match)
                                    class=("cursor-pointer", is_not_match)
                                    class=("hover:underline", is_not_match)
                                    class=("no-underline", is_not_match)
                                >
                                    {display_text}
                                </a>
                            }
                        })
                        .collect_view(cx)}
                </div>
                <Outlet/>
        </div>
    }
}

#[component]
pub fn GreatAt(cx: Scope) -> impl IntoView {
    view! { cx,
        <ul class="list-disc list-outside space-y-4">
            <li>Fullstack web development</li>
            <li>Typescript/Javascript/ES6</li>
            <li>React</li>
            <li>SQL (MySQL, Postgres)</li>
            <li>Designing performant and fault-tolerant RESTful APIs</li>
            <li>Service telemetry and root cause analysis</li>
            <li>Mentoring junior engineers</li>
            <li>Getting it done</li>
        </ul>
    }
}

#[component]
pub fn BetterAt(cx: Scope) -> impl IntoView {
    view! { cx,
        <ul class="list-disc">
            <li>Rust</li>
            <li>Leptos</li>
            <li>Kubernetes</li>
        </ul>
    }
}

#[component]
pub fn InterestedIn(cx: Scope) -> impl IntoView {
    view! { cx,
        <ul class="list-disc">
            <li>Flight sims</li>
            <li>"Motorcyling, Onewheeling, transportation with <= 3 wheels"</li>
            <li>Space - KSP</li>
            <li>WASM</li>
            <li>GoLang</li>
            <li>Aerospace engineering</li>
        </ul>
    }
}
