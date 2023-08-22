use crate::components::about::About;
use crate::components::dev::Dev;
use crate::components::skills::{Beliefs, BetterAt, GreatAt, InterestedIn, Skills};
use leptos::*;
use leptos_meta::{provide_meta_context, Stylesheet, Title};
use leptos_router::{Redirect, Route, Router, Routes};

#[component]
pub fn App(cx: Scope) -> impl IntoView {
    provide_meta_context(cx);

    let routes = vec![("/about", "About"), ("/skills", "Skills"), ("/dev", "Dev")];
    let links = vec![
        ("https://github.com/jaydh", "fa-brands fa-github-square"),
        (
            "https://www.linkedin.com/in/jaydanhoward/",
            "fa-brands fa-linkedin",
        ),
        ("mailto:hello@jaydanhoward.com", "fa-solid fa-envelope"),
    ];

    view! { cx,
        <Stylesheet id="leptos" href="/pkg/leptos_start.css"/>
        <Stylesheet id="fa" href="/assets/fontawesome/css/fontawesome.min.css"/>
        <Stylesheet id="fa-brands" href="/assets/fontawesome/css/brands.min.css"/>
        <Stylesheet id="fa-solid" href="/assets/fontawesome/css/solid.min.css"/>
        <Title text="Jay Dan Howard"/>
        <Router>
            <main>
                <div class="flex w-screen h-full bg-pale-beige px-40">
                    <div class="flex flex-col w-full min-h-screen gap-10 bg-ivory-beige px-40 pb-40">
                        <nav class="pointer-events-auto hidden md:block mt-20 mb-20">
                            <ul class="flex flex-row rounded-full bg-warm-beige px-3 text-sm font-medium">
                                <ul class="flex">
                                    {routes.into_iter()
                                        .map(|(route, display_text)| {
                                            view! { cx,
                                                <a
                                                    href=route
                                                    class="hover:underline relative block px-3 py-2 transition"
                                                >
                                                    {display_text}
                                                </a>
                                            }
                                        })
                                        .collect_view(cx)}
                                </ul>
                                <ul class="flex ml-auto">
                                    {links.into_iter()
                                        .map(|(route, iconClass)| {
                                            view! { cx,
                                                <a
                                                    href=route
                                                    class="hover:underline relative block px-3 py-2 transition"
                                                    target="_blank" rel="noreferrer"
                                                >
                                                    <i class=iconClass />
                                                </a>
                                            }
                                        })
                                        .collect_view(cx)}
                                </ul>
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
                                <Route
                                    path="believe"
                                    view=Beliefs />
                            </Route>
                            <Route path="/dev" view=Dev />
                            <Route path="" view=move |cx| view! { cx, <Redirect path="/about"/> }/>
                            <Route path="/skills" view=move |cx| view! { cx, <Redirect path="/skills/great"/> }/>
                        </Routes>
                    </div>
                </div>
            </main>
        </Router>
    }
}
