use crate::components::about::About;
use crate::components::beliefs::Beliefs;
use crate::components::dev::Dev;
use crate::components::life::Life;
use crate::components::nav::Nav;
use crate::components::resume::Resume;
use crate::components::skills::{Experienced, InterestedIn, Proficient, Skills};
use crate::components::work::Work;
use leptos::*;
use leptos_meta::{provide_meta_context, Stylesheet, Title};
use leptos_router::{Redirect, Route, Router, Routes};

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/leptos_start.css"/>
        <Stylesheet id="fa" href="/assets/fontawesome/css/fontawesome.min.css"/>
        <Stylesheet id="fa-brands" href="/assets/fontawesome/css/brands.min.css"/>
        <Stylesheet id="fa-solid" href="/assets/fontawesome/css/solid.min.css"/>
        <Title text="Jay Dan Howard"/>
        <Router>
            <main>
                <div class="flex flex-col w-screen h-screen bg-charcoal text-white">
                    <Nav/>
                    <div class="grow flex flex-col w-full gap-10 items-center">
                        <Routes>
                            <Route
                                path="/about"
                                view=move || view! { <Redirect path="/about/1"/> }
                            />
                            <Route path="/about/4" view=move || view! { <Redirect path="/about/4/skills"/> } />
                            <Route path="/about/:section" view=About>
                                <Route path="skills" view=Skills>
                                    <Route path="experienced" view=Experienced/>
                                    <Route path="proficient" view=Proficient/>
                                    <Route path="interested" view=InterestedIn/>
                                    <Route path="/" view=move || view! { <Redirect path="experienced"/> }/>
                                </Route>
                                <Route path="beliefs" view=Beliefs/>
                                <Route path="/*any" view=|| ()/>
                            </Route>
                            <Route path="/dev" view=Dev/>
                            <Route path="/work" view=Work>
                                <Route path="/life" view=Life/>
                                <Route path="/" view=move || view! { <Redirect path="life"/> }/>
                            </Route>
                            <Route path="/resume" view=Resume/>
                            <Route path="/*any" view=move || view! { <Redirect path="/about"/> }/>
                        </Routes>
                    </div>
                </div>
            </main>
        </Router>
    }
}
