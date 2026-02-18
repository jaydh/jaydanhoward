use crate::components::about::About;
use crate::components::footer::Footer;
use crate::components::life::Life;
use crate::components::nav::Nav;
use crate::components::path_search::PathSearch;
use crate::components::photography::Photography;
use crate::components::satellite_tracker::SatelliteTracker;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Html, Link, Script, Style, Title};
use leptos_router::components::*;
use leptos_router::path;

#[cfg(feature = "ssr")]
fn get_inlined_css() -> String {
    use runfiles::{rlocation, Runfiles};
    use std::fs;

    let r = Runfiles::create().expect("Must run using bazel with runfiles");
    let css_path = rlocation!(r, "_main/assets/style.css").expect("Failed to locate style.css");
    fs::read_to_string(css_path).expect("Failed to read style.css")
}

#[component]
fn Stylesheets() -> impl IntoView {
    #[cfg(feature = "ssr")]
    {
        let css_content = get_inlined_css();
        view! {
            <Style id="app-styles">{css_content}</Style>
        }
    }

    #[cfg(not(feature = "ssr"))]
    {
        view! {
            <Style id="app-styles"></Style>
        }
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
            <section id="satellites" class="flex flex-col py-20 border-t border-border">
                <div class="max-w-7xl mx-auto px-8 w-full flex flex-col gap-8 items-center">
                    <h1 class="text-3xl font-bold text-charcoal">
                        "Satellites"
                    </h1>
                    <SatelliteTracker />
                </div>
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
            <Footer />
        </div>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheets />
        <Script>{r#"
            (function(){
                // Default to dark mode if no preference is set
                if(localStorage.theme==='dark'||!('theme' in localStorage)||(localStorage.theme!=='light'&&window.matchMedia('(prefers-color-scheme: dark)').matches)){
                    document.documentElement.classList.add('dark')
                }
            })()
        "#}</Script>
        <Link rel="shortcut icon" type_="image/ico" href="/assets/favicon.ico" />
        <Link rel="preconnect" href="https://caddy.jaydanhoward.com" />
        <Link rel="dns-prefetch" href="https://caddy.jaydanhoward.com" />
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
