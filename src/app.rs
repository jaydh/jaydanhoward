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
                <Routes>
                    <Route path="/about" view=HomePage>
                        <Route path="great" view= |cx| view!{ cx,
                            <ul>
                                <li>Fullstack web development </li>
                                <li>Typescript/Javascript/ES6</li>
                                <li>React</li>
                                <li>SQL (MySQL, Postgres)</li>
                                <li>Designing performant and fault-tolerant RESTful APIs</li>
                                <li>Service telemetry and root cause analysis</li>
                                <li>Mentoring junior engineers</li>
                                <li>Getting it done</li>
                            </ul>
                        }/>
                        <Route path="better" view= |cx| view!{ cx,
                            <ul>
                                <li>Rust</li>
                                <li>Leptos</li>
                                <li>Kubernetes</li>
                            </ul>
                        }/>
                        <Route path="interested" view= |cx| view!{ cx,
                            <ul>
                                <li>Flight sims</li>
                                <li>r#"Motorcyling, Onewheeling, transportation with <= 3 wheels"#</li>
                                <li>Space - KSP</li>
                                <li>WASM</li>
                                <li>GoLang</li>
                                <li>Aerospace engineering</li>
                            </ul>
                        }/>
                    </Route>
                    <Route path="" view=move |cx| view! { cx, <Redirect path="/about/great"/> } />

                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage(cx: Scope) -> impl IntoView {
    let location = use_location(cx);

    view! { cx,
        <h1>"👋I'm Jay!"</h1>
        <h2>"I deeply care about technology that deeply cares about people."</h2>
        <div class="about">
            <div class="about-text">
            <p>"Currently a senior software engineer at Interwell Health, leading an engineering team where we use software to empower clinicians and nephrologists to treat and prevent kidney disease. I try to keep a low-key life and avoid the spotlight but with that said, I plan to change the world."</p>
            <div class="about-nav">
                r#"Things I'm "#
                <a
                    href="great"
                    class="about-nav-item"
                    class=("about-nav-item-selected", move || location.pathname.get() == "/about/great")>
                    great at
               </a>
                <a
                    href="better"
                    class="about-nav-item"
                    class=("about-nav-item-selected", move || location.pathname.get() == "/about/better")>
                    getting better at
               </a>
                <a
                    href="interested"
                    class="about-nav-item"
                    class=("about-nav-item-selected", move || location.pathname.get() == "/about/interested")>
                     interested in
                </a>
            </div>
            <Outlet/>
            </div>
            <img src="/assets/profile.jpg" />
        </div>
    }
}
