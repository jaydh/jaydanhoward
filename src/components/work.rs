use crate::components::binary_nav_switcher::BinaryNavSwithcer;
use leptos::prelude::*;
use leptos_router::nested_router::Outlet;

#[component]
pub fn Work() -> impl IntoView {
    view! {
        <div class="w-5/6 mg:w-4/6 lg:w-1/2 p-10 grow flex flex-col items-center text-lg scroll-smooth items-center space-y-10">
            <BinaryNavSwithcer
                a_path="projects"
                a_display_text="Projects"
                b_path="dev"
                b_display_text="Dev"
            />
            <Outlet />
        </div>
    }
}
