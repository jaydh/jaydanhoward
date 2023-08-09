use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[component]
pub fn App(cx: Scope) -> impl IntoView {
    provide_meta_context(cx);

    view! { cx,
        <Stylesheet id="leptos" href="/pkg/leptos_start.css"/>

        <Title text="Jay Dan Howard"/>
        <link
            href="https://fonts.googleapis.com/css?family=Press+Start+2P&display=swap"
            rel="stylesheet"
        />

        <Router>
            <main>
                <Routes>
                    <Route path="" view=HomePage/>
                    <Route path="/*any" view=NotFound/>
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage(cx: Scope) -> impl IntoView {
    view! { cx,
        <h1>"Hi, I'm Jay!"</h1>
        <div>
            "Currently a senior software engineer at Cricket Health where we use software to empower clinicians and nephrologists to treat and prevent kidney disease."
        </div>
        <div>
            "I'm passionate making software that helps those in need, particularly in health care and in education (I've coached high school debate and tutored programming)."
        </div>
        <div>
            "During the pandemic I picked up a few new hobbies like flight sims (in VR), 3D printing, and Onewheeling . I currently spend a lot of my time practicing flying the Huey with an online community that has real military aviators! From their coaching I'm pretty sure I could fly one in real life in a pinch, though I hope that never gets put to the test. I'm also very passionate about space exploration and think we should never stop exploring.
            "
        </div>
        <div>
            "I try to keep a low-key life and avoid the spotlight but with that said, I plan to change the world."
        </div>
    }
}

/// 404 - Not Found
#[component]
fn NotFound(cx: Scope) -> impl IntoView {
    // set an HTTP status code 404
    // this is feature gated because it can only be done during
    // initial server-side rendering
    // if you navigate to the 404 page subsequently, the status
    // code will not be set because there is not a new HTTP request
    // to the server
    #[cfg(feature = "ssr")]
    {
        // this can be done inline because it's synchronous
        // if it were async, we'd use a server function
        let resp = expect_context::<leptos_actix::ResponseOptions>(cx);
        resp.set_status(actix_web::http::StatusCode::NOT_FOUND);
    }

    view! { cx, <h1>"Not Found"</h1> }
}
