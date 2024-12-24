use crate::components::about::About;
use crate::components::dark_mode_toggle::initial_prefers_dark;
use crate::components::dev::Dev;
use crate::components::life::Life;
use crate::components::nav::Nav;
use crate::components::path_search::PathSearch;
use crate::components::projects::Projects;
use crate::components::work::Work;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Html, Link, Meta, Stylesheet, Title};
use leptos_router::components::*;
use leptos_router::path;

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
    view! { <FontAwesomeCss /> }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    let (dark_mode_enabled, set_dark_mode_enabled) = create_signal(initial_prefers_dark());

    view! {
        <head>
            <Meta
                name="description"
                content="Welcome to Jay Dan Howards's Portfolio | Full-Stack Software Engineer in Health-Tech | Exploring Rust - Explore my projects, expertise, and journey in health-tech development. Discover how I leverage my skills to innovate and create in the world of health technology, with a passion for learning Rust"
            />
        </head>
        <Stylesheet href="/assets/style.css" />
        <FontAwesome />
        <Link rel="shortcut icon" type_="image/ico" href="/assets/favicon.ico" />
        <Title text="Jay Dan Howard" />
        <div
            id="root"
            class="flex flex-col min-w-screen min-h-screen bg-gray text-charcoal dark:bg-charcoal dark:text-gray"
        >
            <Router>
                <main>
                    <Html
                        {..}
                        lang="he"
                        dir="rtl"
                        class=move || {
                            match dark_mode_enabled() {
                                true => "dark",
                                false => "light",
                            }
                        }
                    />
                    <Nav set_dark_mode_enabled=set_dark_mode_enabled />
                    <div class="overflow-y-auto grow flex flex-col w-full gap-10 items-center">
                        <Routes fallback=|| "Not found">
                            <Route
                                path=path!("/")
                                view=move || view! { <Redirect path="/about" /> }
                            />
                            <Route path=path!("about") view=About />
                            <ParentRoute path=path!("work") view=Work>
                                <Route
                                    path=path!("")
                                    view=move || view! { <Redirect path="/work/dev" /> }
                                />
                                <Route path=path!("dev") view=Dev />
                                <ParentRoute path=path!("projects") view=Projects>
                                    <Route path=path!("life") view=Life />
                                    <Route path=path!("path") view=PathSearch />
                                    <Route
                                        path=path!("")
                                        view=move || {
                                            view! { <Redirect path="/work/projects/life" /> }
                                        }
                                    />
                                </ParentRoute>
                            </ParentRoute>
                        </Routes>
                    </div>
                </main>
            </Router>
        </div>
    }
}
