use crate::components::about::About;
use crate::components::beliefs::Beliefs;
use crate::components::dark_mode_toggle::initial_prefers_dark;
use crate::components::dev::Dev;
use crate::components::life::Life;
use crate::components::nav::Nav;
use crate::components::path_search::PathSearch;
use crate::components::projects::Projects;
use crate::components::resume::Resume;
use crate::components::skills::{Experienced, InterestedIn, Proficient, Skills};
use crate::components::work::Work;
use leptos::*;
use leptos_meta::{provide_meta_context, Html, Link, Meta, Stylesheet, Title};
use leptos_router::{Redirect, Route, Router, Routes};

#[component]
fn DarkAwareHTML(dark_mode_enabled: ReadSignal<bool>) -> impl IntoView {
    view! {
        <Html
            lang="en"
            class=move || {
                match dark_mode_enabled() {
                    true => "dark",
                    false => "light",
                }
            }
        />
    }
}

#[component]
fn FontAwesomeCss() -> impl IntoView {
    #[cfg(not(debug_assertions))] // release mode
    {
        view! {
            <script
                src="https://kit.fontawesome.com/6ae5d22557.js"
                crossorigin="anonymous"
                async="true"
            ></script>
        }
    }

    #[cfg(debug_assertions)] // dev mode
    {
        view! {
            <Link rel="preload" href="/assets/fontawesome/css/fontawesome.min.css" as_="style" />
            <Link rel="preload" href="/assets/fontawesome/css/brands.min.css" as_="style" />
            <Link rel="preload" href="/assets/fontawesome/css/solid.min.css" as_="style" />
            <Stylesheet id="fa" href="/assets/fontawesome/css/fontawesome.min.css" />
            <Stylesheet id="fa-brands" href="/assets/fontawesome/css/brands.min.css" />
            <Stylesheet id="fa-solid" href="/assets/fontawesome/css/solid.min.css" />
        }
    }
}

#[component]
fn FontAwesome() -> impl IntoView {
    view! {
        <FontAwesomeCss  />
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    let (dark_mode_enabled, set_dark_mode_enabled) = create_signal(initial_prefers_dark());

    view! {
        <DarkAwareHTML dark_mode_enabled=dark_mode_enabled />
        <Meta
            name="description"
            content="Welcome to Jay Dan Howards's Portfolio | Full-Stack Software Engineer in Health-Tech | Exploring Rust - Explore my projects, expertise, and journey in health-tech development. Discover how I leverage my skills to innovate and create in the world of health technology, with a passion for learning Rust"
        />
        <Stylesheet id="leptos" href="/pkg/leptos_start.css" />
        <FontAwesome />
        <Link rel="shortcut icon" type_="image/ico" href="/assets/favicon.ico" />
        <Title text="Jay Dan Howard" />
        <Router trailing_slash=leptos_router::TrailingSlash::Redirect>
            <main>
                <div class="flex flex-col min-w-screen min-h-screen bg-gray text-charcoal dark:bg-charcoal dark:text-gray">
                    <Nav set_dark_mode_enabled=set_dark_mode_enabled />
                    <div class="overflow-y-auto grow flex flex-col w-full gap-10 items-center">
                        <Routes>
                            <Route
                                path="/about"
                                view=move || view! { <Redirect path="/about/1" /> }
                            />
                            <Route
                                path="/about/4"
                                view=move || view! { <Redirect path="/about/4/skills" /> }
                            />
                            <Route path="/about/:section" view=About>
                                <Route path="skills" view=Skills>
                                    <Route path="experienced" view=Experienced />
                                    <Route path="proficient" view=Proficient />
                                    <Route path="interested" view=InterestedIn />
                                    <Route
                                        path="/*any"
                                        view=move || view! { <Redirect path="experienced" /> }
                                    />
                                </Route>
                                <Route path="beliefs" view=Beliefs />
                                <Route path="/*any" view=|| () />
                            </Route>
                            <Route path="/about/:section" view=About>
                                <Route path="skills" view=Skills>
                                    <Route path="experienced" view=Experienced />
                                    <Route path="proficient" view=Proficient />
                                    <Route path="interested" view=InterestedIn />
                                    <Route
                                        path="/*any"
                                        view=move || view! { <Redirect path="experienced" /> }
                                    />
                                </Route>
                                <Route path="beliefs" view=Beliefs />
                                <Route path="/*any" view=|| () />
                            </Route>
                            <Route path="/work" view=Work>
                                <Route path="dev" view=Dev />
                                <Route path="projects" view=Projects>
                                    <Route path="life" view=Life />
                                    <Route path="path" view=PathSearch />
                                    <Route
                                        path="/"
                                        view=move || view! { <Redirect path="life" /> }
                                    />
                                </Route>
                                <Route
                                    path="/*any"
                                    view=move || view! { <Redirect path="dev" /> }
                                />
                            </Route>
                            <Route path="/resume" view=Resume />
                            <Route path="/*any" view=move || view! { <Redirect path="/about" /> } />
                        </Routes>
                    </div>
                </div>
            </main>
        </Router>
    }
}
