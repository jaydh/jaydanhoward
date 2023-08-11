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
                    <Route path="/*any" view=HomePage/>
                    <Route path="/about" view=HomePage>
                        <Route path="great" view= |cx| view!{ cx,
                            <ul>
                                <li>Fullstack web development </li>
                                <li>Typescript/Javascript/ES6</li>
                                <li>React</li>
                                <li>SQL (MySQL, Postgres)</li>
                                <li>Designing fault-tolerant RESTful APIs</li>
                            </ul>
                        }/>
                        <Route path="better" view= |cx| view!{ cx,
                            <ul>
                                <li>Rust</li>
                                <li>Kubernetes</li>
                            </ul>
                        }/>
                        <Route path="interested" view= |cx| view!{ cx,
                            <ul>
                                <li>Flight sims</li>
                                <li>r#"Motorcyling, Onewheeling, transportation with <= 3 wheels"#</li>
                                <li>Flight sims</li>
                                <li>Space - KSP</li>
                            </ul>
                        }/>
                    </Route>

                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage(cx: Scope) -> impl IntoView {
    view! { cx,
        <h1>"Hi, I'm Jay!"</h1>
        <div class="about">
            <div>
            "Currently a senior software engineer at Interwell Health leading an engineering team where we use software to empower clinicians and nephrologists to treat and prevent kidney disease. I care mostly about technology that mostly cares about people."
            </div>
            <img href="profile.jpg"/>
        </div>
        <div>
            "I try to keep a low-key life and avoid the spotlight but with that said, I plan to change the world."
        </div>
        <div>
            r#"Things I'm "#
            <a href="great">great at:</a>
            <a href="better">getting better at:</a>
            <a href="interested">interested in:</a>
            <Outlet/>
        </div>
    }
}
