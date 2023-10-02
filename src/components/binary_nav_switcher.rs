use crate::components::link::Link;
use leptos::*;

#[component]
pub fn BinaryNavSwithcer(
    a_path: &'static str,
    a_display_text: &'static str,
    b_path: &'static str,
    b_display_text: &'static str,
) -> impl IntoView {
    view! {
        <div class="w-full flex flex-row gap-10 mb-20 items-center">
            <Link path=a_path display_text=a_display_text/>
            <div class="grow h-px bg-charcoal dark:bg-gray mx-4"></div>
            <Link path=b_path display_text=b_display_text/>
        </div>
    }
}
