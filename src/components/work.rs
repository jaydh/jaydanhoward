use crate::components::binary_nav_switcher::BinaryNavSwithcer;
use leptos::*;
use leptos_router::Outlet;

#[component]
pub fn Work() -> impl IntoView {
    view! {
        <div class="w-1/2 p-10 grow flex flex-col text-white text-lg scroll-smooth items-center space-y-10">
            <BinaryNavSwithcer
                a_path="projects"
                a_display_text="Projects"
                b_path="dev"
                b_display_text="Dev"
            />
            <Outlet/>
        </div>
    }
}
