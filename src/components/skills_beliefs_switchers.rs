use leptos::*;
use leptos_router::{use_location, use_route};

#[component]
pub fn SkillsBeliefsSwitcher() -> impl IntoView {
    let location = use_location();
    let route = use_route();
    let pathname = move || location.pathname.get();

    let routes = vec![("skills/great", "Skills"), ("beliefs", "Beliefs")];

    view! {
            <div>
                <div class="flex flex-row gap-10 mb-20">
                    {routes.into_iter()
                        .map(|(r, display_text)| {
                            let is_match = move || pathname().contains(r);
                            let is_not_match = move || !is_match();

                            view! {
                                <a
                                    href={route.resolve_path(r)}
                                    class=("underline", is_match)
                                    class=("font-bold", is_match)
                                    class=("cursor-default", is_match)
                                    class=("cursor-pointer", is_not_match)
                                    class=("hover:underline", is_not_match)
                                    class=("no-underline", is_not_match)
                                    class=("font-medium", is_not_match)
                                >
                                    {display_text}
                                </a>
                            }
                        })
                        .collect_view()}
                </div>
        </div>
    }
}
