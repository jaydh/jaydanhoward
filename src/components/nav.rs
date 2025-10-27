use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate};

#[component]
pub fn Nav() -> impl IntoView {
    let location = use_location();
    let pathname = move || location.pathname.get();
    let navigate = use_navigate();

    let routes = vec![("/about", "About"), ("/work", "Projects"), ("/photography", "Photography")];
    let contact_links = vec![
        (
            "https://github.com/jaydh",
            "fa-brands fa-github-square",
            true,
        ),
        (
            "https://www.linkedin.com/in/jaydanhoward/",
            "fa-brands fa-linkedin",
            true,
        ),
        (
            "mailto:hello@jaydanhoward.com",
            "fa-solid fa-envelope",
            true,
        ),
    ];

    let (show_contact_links, set_show_contact_links) = signal(false);

    view! {
        <nav class="sticky top-0 flex flex-row pointer-events-auto px-8 py-6 text-base border-b border-border bg-surface bg-opacity-80 backdrop-blur-md z-40">
            <div class="flex items-center gap-8 max-w-7xl mx-auto w-full">
                <a
                    href="/"
                    class="font-bold text-lg tracking-tight hover:text-accent transition-colors duration-200"
                    on:click=move |_| {
                        set_show_contact_links.set(false);
                    }
                >

                    Jay Dan Howard
                </a>
                <div class="flex ml-auto items-center gap-1">
                    {routes
                        .into_iter()
                        .map(|(route, display_text)| {
                            let nav = navigate.clone();
                            view! {
                                <button
                                    type="button"
                                    class="px-4 py-2 relative transition-all duration-200"
                                    class=(
                                        "text-accent",
                                        move || {
                                            let path = pathname();
                                            path == route || path.starts_with(&format!("{}/", route))
                                        },
                                    )

                                    class=(
                                        "after:absolute after:bottom-0 after:left-0 after:right-0 after:h-0.5 after:bg-accent after:scale-x-100",
                                        move || {
                                            let path = pathname();
                                            path == route || path.starts_with(&format!("{}/", route))
                                        },
                                    )

                                    class=(
                                        "text-charcoal hover:text-accent",
                                        move || {
                                            let path = pathname();
                                            !(path == route || path.starts_with(&format!("{}/", route)))
                                        },
                                    )

                                    class=(
                                        "after:absolute after:bottom-0 after:left-0 after:right-0 after:h-0.5 after:bg-accent after:scale-x-0 hover:after:scale-x-100 after:transition-transform after:duration-200",
                                        move || {
                                            let path = pathname();
                                            !(path == route || path.starts_with(&format!("{}/", route)))
                                        },
                                    )

                                    on:click=move |_| {
                                        set_show_contact_links.set(false);
                                        nav(route, Default::default());
                                    }
                                >

                                    {display_text}
                                </button>
                            }
                        })
                        .collect_view()} <div class="flex flex-col relative">
                        <button
                            type="button"
                            class="px-4 py-2 text-charcoal hover:text-accent transition-colors duration-200"
                            on:click=move |_| {
                                set_show_contact_links.set(!show_contact_links());
                            }
                        >

                            Contact
                            <i class="fas fa-caret-down ml-1 text-xs"></i>
                        </button>
                        <div class="absolute top-full mt-2 right-0 z-10">
                            <Show when=move || {
                                show_contact_links()
                            }>
                                <div class="bg-surface border border-border rounded-lg shadow-minimal-lg overflow-hidden min-w-[160px]">
                                    {contact_links
                                        .clone()
                                        .into_iter()
                                        .map(|(route, iconClass, external)| {
                                            let target = if external { "_blank" } else { "_self" };
                                            view! {
                                                <a
                                                    href=route
                                                    class="hover:bg-border hover:bg-opacity-50 hover:text-accent flex items-center gap-3 px-4 py-3 transition-colors duration-200"
                                                    target=target
                                                    rel="noreferrer"
                                                >

                                                    <i class=iconClass></i>
                                                </a>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </Show>
                        </div>
                    </div>
                </div>
            </div>
        </nav>
    }
}
