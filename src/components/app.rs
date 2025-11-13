use crate::components::about::About;
use crate::components::life::Life;
use crate::components::nav::Nav;
use crate::components::path_search::PathSearch;
use crate::components::photography::Photography;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Html, Link, Stylesheet, Title};
use leptos_router::components::*;
use leptos_router::path;

#[component]
fn Stylesheets() -> impl IntoView {
    view! {
        <Stylesheet id="app-styles" href="/assets/style.css" />
    }
}

#[component]
fn HomePage() -> impl IntoView {
    view! {
        <div
            id="main-scroll-container"
            class="overflow-y-scroll grow"
            style="scroll-behavior: smooth; -webkit-overflow-scrolling: touch; will-change: scroll-position;"
        >
            <section id="about" class="flex flex-col py-20">
                <About />
            </section>
            <section id="life" class="flex flex-col py-20 border-t border-border">
                <Life />
            </section>
            <section id="path" class="flex flex-col py-20 border-t border-border">
                <PathSearch />
            </section>
            <section id="photography" class="flex flex-col py-20 border-t border-border">
                <Photography />
            </section>
        </div>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheets />
        <Link rel="shortcut icon" type_="image/ico" href="/assets/favicon.ico" />
        <Title text="Jay Dan Howard" />
        <main>
            <Html {..} />
            <div
                id="root"
                class="flex flex-col min-w-screen h-screen bg-surface text-charcoal"
            >
                <Router>
                    <Nav />
                    <Routes fallback=|| "Not found">
                        <Route path=path!("/") view=HomePage />
                    </Routes>
                </Router>
            </div>
        </main>
    }
}
