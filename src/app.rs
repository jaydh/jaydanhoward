use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[component]
pub fn App(cx: Scope) -> impl IntoView {
    provide_meta_context(cx);

    view! { cx,
        <Stylesheet id="leptos" href="/pkg/leptos_start.css"/>

        <Title text="Jay Dan Howard"/>
        <Router>
            <main>
                <div class="flex w-screen h-screen bg-pale-beige px-40">
                    <div class="flex flex-col gap-10 bg-ivory-beige px-40">
                        <nav class="pointer-events-auto hidden md:block mt-20 mb-28">
                            <ul class="flex rounded-full bg-warm-beige px-3 text-sm font-medium">
                                <li><a class="hover:underline relative block px-3 py-2 transition" href="/about">About</a></li>
                                <li><a class="hover:underline relative block px-3 py-2 transition" href="/skills">Skills</a></li>
                            </ul>
                        </nav>
                        <Routes>

                            <Route path="/about" view=About/ >
                            <Route path="/skills" view=Skills>
                                <Route
                                    path="great"
                                    view=GreatAt
                                />
                                <Route
                                    path="better"
                                    view=BetterAt />
                                <Route
                                    path="interested"
                                    view=InterestedIn />
                            </Route>
                            <Route path="" view=move |cx| view! { cx, <Redirect path="/about"/> }/>
                            <Route path="/skills" view=move |cx| view! { cx, <Redirect path="/skills/great"/> }/>
                        </Routes>
                    </div>
                </div>
            </main>
        </Router>
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

#[component]
pub fn About(cx: Scope) -> impl IntoView {
    view! {
        cx,
        <div class="flex flex-row">
            <div>
                <h1 class="text-5xl font-heavy mb-6">r#"👋I'm Jay Dan Howard! I believe compassion makes tech worthwhile."#</h1>
                <p>"Very few things are good in and of themselves, and tech is probably not one of them. I'm currently a senior software engineer at Interwell Health, leading an engineering team where we use software to empower clinicians and nephrologists to treat and prevent kidney disease. I try to keep a low-key life and avoid the spotlight but with that said, I plan to change the world."
                </p>
            </div>
            <img src="/assets/profile.jpg" class="pl-20"/>
        </div>

    }
}

#[component]
fn Skills(cx: Scope) -> impl IntoView {
    let location = use_location(cx);
    let pathname = move || location.pathname.get();
    let is_great_link = move || pathname() == "/skills/great";
    let is_better_link = move || pathname() == "/skills/better";
    let is_interested_link = move || pathname() == "/skills/interested";

    view! { cx,
            <div>
                <div class="flex flex-row gap-10 mb-20">
                    <span>"Things I'm"</span>
                    <a
                        href="great"
                        class=("underline", move || is_great_link())
                        class=("font-heavy", move || is_great_link())
                        class=("cursor-default", move || is_great_link())
                        class=("cursor-pointer", move || !is_great_link())
                        class=("hover:underline", move || !is_great_link())
                        class=("no-underline", move || !is_great_link())
                    >
                           great at
                    </a>
                    <a
                        href="better"
                        class=("underline", move || is_better_link())
                        class=("font-heavy", move || is_better_link())
                        class=("cursor-default", move || is_better_link())
                        class=("cursor-pointer", move || !is_better_link())
                        class=("hover:underline", move || !is_better_link())
                        class=("no-underline", move || !is_better_link())
                    >
                           getting better at

                    </a>
                    <a
                        href="interested"
                        class=("underline", move || is_interested_link())
                        class=("font-heavy", move || is_interested_link())
                        class=("cursor-default", move || is_interested_link())
                        class=("cursor-pointer", move || !is_interested_link())
                        class=("hover:underline", move || !is_interested_link())
                        class=("no-underline", move || !is_interested_link())
                    >
                           interested in
                    </a>
                    <span>:</span>
                </div>
                <Outlet/>
        </div>
    }
}
