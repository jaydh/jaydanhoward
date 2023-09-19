use crate::components::about::About;
use crate::components::dev::Dev;
use crate::components::life::Life;
use crate::components::nav::Nav;
use crate::components::resume::Resume;
use crate::components::skills::{Beliefs, BetterAt, GreatAt, InterestedIn, Skills};
use crate::components::work::Work;
use leptos::*;
use leptos_meta::{provide_meta_context, Stylesheet, Title};
use leptos_router::{Redirect, Route, Router, Routes};

#[component]
pub fn App(cx: Scope) -> impl IntoView {
    provide_meta_context(cx);

    view! { cx,
        <Stylesheet id="leptos" href="/pkg/leptos_start.css"/>
        <Stylesheet id="fa" href="/assets/fontawesome/css/fontawesome.min.css"/>
        <Stylesheet id="fa-brands" href="/assets/fontawesome/css/brands.min.css"/>
        <Stylesheet id="fa-solid" href="/assets/fontawesome/css/solid.min.css"/>
        <Title text="Jay Dan Howard"/>
        <Router>
            <main>
                <div class="flex flex-col w-screen h-full bg-charcoal text-white">
                    <Nav/>
                    <div class="flex flex-col w-full min-h-screen gap-10 items-center">
                        <Routes>
                            <Route path="/about" view=About/>
                            <Route path="/skills" view=Skills>
                                <Route path="great" view=GreatAt/>
                                <Route path="better" view=BetterAt/>
                                <Route path="interested" view=InterestedIn/>
                                <Route path="believe" view=Beliefs/>
                                <Route
                                    path="/"
                                    view=move |cx| view! { cx, <Redirect path="great"/> }
                                />
                            </Route>
                            <Route path="/dev" view=Dev/>
                            <Route path="/work" view=Work>
                                <Route path="/life" view=Life/>
                                <Route
                                    path="/"
                                    view=move |cx| view! { cx, <Redirect path="life"/> }
                                />
                            </Route>
                            <Route path="/resume" view=Resume/>
                            <Route path="" view=move |cx| view! { cx, <Redirect path="/about"/> }/>
                        </Routes>
                    </div>
                </div>
            </main>
        </Router>
    }
}
