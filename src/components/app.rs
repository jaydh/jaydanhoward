use crate::components::about::About;
use crate::components::skills::{BetterAt, GreatAt, InterestedIn, Skills};
use leptos::*;
use leptos_meta::{provide_meta_context, Stylesheet, Title};
use leptos_router::{Redirect, Route, Router, Routes};

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
