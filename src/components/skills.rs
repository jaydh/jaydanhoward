use crate::components::source_anchor::SourceAnchor;
use leptos::*;
use leptos_router::{use_location, Outlet};

#[component]
pub fn Skills(cx: Scope) -> impl IntoView {
    let location = use_location(cx);
    let pathname = move || location.pathname.get();

    let routes = vec![
        ("great", "am great at"),
        ("better", "am getting better at"),
        ("interested", "am interested in"),
        ("believe", "believe in"),
    ];

    view! { cx,
            <SourceAnchor href="#[git]" />
            <div>
                <div class="flex flex-row gap-10 mb-20">
                    <span>"Things I"</span>
                    {routes.into_iter()
                        .map(|(route, display_text)| {
                            let is_match = move || pathname() == format!("/skills/{}", route);
                            let is_not_match = move || !is_match();

                            view! { cx,
                                <a
                                    href={route}
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
            <li>Leading teams</li>
            <li>Talking to stake holders</li>
            <li>Finding high value wins (especially the ones with low costs)</li>
            <li>Mentoring junior engineers</li>
            <li>Getting it done</li>
        </ul>
    }
}

#[component]
pub fn BetterAt(cx: Scope) -> impl IntoView {
    view! { cx,
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
pub fn InterestedIn(cx: Scope) -> impl IntoView {
    view! { cx,
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

#[component]
pub fn Beliefs(cx: Scope) -> impl IntoView {
    view! { cx,
        <h2 class="mb-10 font-semibold">Evolving list of loosely held, hopefully not pretentious sounding, and strong beliefs about software engineering, mayhaps only applying to the web world: </h2>
        <ul class="list-disc list-outside space-y-4 mb-60">
            <li>Code that feels good to write is productive code</li>
            <li>Teams should use auto-formatters</li>
            <li>Unit test files that have more lines of mocks than tests is a really bad sign since they cement contracts that will be null and void when business needs and implementations change</li>
            <li>Dogma is as rampant in tech as it is anywhere else people are involved. We should always stay open-minded</li>
            <li>r#"Everything in software is about getting things to communicate "well""#</li>
            <ul class="list-none list-outside pl-8 space-y-4">
                <li>r#"▫️ To people: other developers, business stakeholders, users, API consumers"#</li>
                <li>r#"▫️ To machines: clients to services, services to services, processes to processes"#</li>
                <li>r#"▫️ To present-tense code: interfaces between functions, classes, network calls"#</li>
                <li>r#"▫️ To past-tense code: gracefully handling old versions of code after deploying to prod"#</li>
                <li>r#"▫️ To future-tense code: today's code will happily swap in and out with tomorrow's code"#</li>
            </ul>
            <li>r#"We need to remember performance is a measurement of the latency and throughput of the communication between or within machines"#</li>
            <li>r#"Performance claims that aren't measurable tend to be a result of engineers being shy about being called artists. We should not be shy"#</li>
            <li>Large multi-hundred-line PRs indicate problems in the process</li>
            <ul class="list-none list-outside pl-8 space-y-4">
                <li>r#"▫️ Poor developer utilization"#</li>
                <li>r#"▫️ Slower and more painful feedback cycles (requested changes have to happen in the service code and in tests)"#</li>
                <li>r#"▫️ Behind-the-scene iteration from single developer battling dragons"#</li>
            </ul>
        </ul>
    }
}
