use leptos::*;

#[component]
pub fn Nav(cx: Scope) -> impl IntoView {
    let routes = vec![("/work", "Work"), ("/about", "About")];
    let contact_links = vec![
        ("/resume", "fa-regular fa-file-lines", false),
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

    let (show_contact_links, set_show_contact_links) = create_signal(cx, false);

    view! { cx,
        <nav class="sticky flex flex-row pointer-events-auto m-20">
            <a
                href="/"
                class="hover:underline px-3 py-2 transition"
                on:click=move |_| {
                    set_show_contact_links.set(false);
                }
            >

                Jay Dan Howard
            </a>
            <ul class="flex ml-auto">
                {routes
                    .into_iter()
                    .map(|(route, display_text)| {
                        view! { cx,
                            <a
                                href=route
                                class="hover:underline px-3 py-2 transition"
                                on:click=move |_| {
                                    set_show_contact_links.set(false);
                                }
                            >

                                {display_text}
                            </a>
                        }
                    })
                    .collect_view(cx)} <div class="flex flex-col">
                    <button
                        type="button"
                        class="hover:underline px-3 py-2 transition"
                        on:click=move |_| {
                            set_show_contact_links.set(!show_contact_links());
                        }
                    >

                        Contact
                        <i class="fas fa-caret-down"></i>
                    </button>
                    <div class="absolute mt-10">
                        <Show when=move || { show_contact_links() == true } fallback=|_| ()>
                            {contact_links
                                .clone()
                                .into_iter()
                                .map(|(route, iconClass, external)| {
                                    let target = if external { "_blank" } else { "_self" };
                                    view! { cx,
                                        <a
                                            href=route
                                            class="hover:underline relative block px-3 py-2 transition"
                                            target=target
                                            rel="noreferrer"
                                        >

                                            <i class=iconClass></i>
                                        </a>
                                    }
                                })
                                .collect_view(cx)}
                        </Show>
                    </div>
                </div>
            </ul>
        </nav>
    }
}
