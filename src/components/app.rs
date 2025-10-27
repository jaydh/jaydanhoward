use crate::components::about::About;
use crate::components::life::Life;
use crate::components::nav::Nav;
use crate::components::path_search::PathSearch;
use crate::components::photography::Photography;
use crate::components::work::Work;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Html, Link, Stylesheet, Title};
use leptos_router::components::*;
use leptos_router::path;

#[component]
fn Stylesheets() -> impl IntoView {
    view! {
        <Link rel="preload" href="/assets/style.css" as_="style" />
        <Link rel="preload" href="/assets/fontawesome/css/fontawesome.min.css" as_="style" />
        <Link rel="preload" href="/assets/fontawesome/css/brands.min.css" as_="style" />
        <Link rel="preload" href="/assets/fontawesome/css/solid.min.css" as_="style" />
        <Stylesheet id="app-styles" href="/assets/style.css" />
        <Stylesheet id="fa" href="/assets/fontawesome/css/fontawesome.min.css" />
        <Stylesheet id="fa-brands" href="/assets/fontawesome/css/brands.min.css" />
        <Stylesheet id="fa-solid" href="/assets/fontawesome/css/solid.min.css" />
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
                class="flex flex-col min-w-screen min-h-screen bg-surface text-charcoal"
            >
                <Router>
                    <Nav />
                    <div class="overflow-y-auto grow flex flex-col w-full">
                        <Routes fallback=|| "Not found">
                            <Route
                                path=path!("/")
                                view=move || view! { <Redirect path="/about" /> }
                            />
                            <Route path=path!("about") view=About />
                            <Route path=path!("work") view=Work />
                            <Route path=path!("work/life") view=Life />
                            <Route path=path!("work/path") view=PathSearch />
                            <Route path=path!("photography") view=Photography />
                        </Routes>
                    </div>
                </Router>
            </div>
        </main>
    }
}
