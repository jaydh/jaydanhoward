use crate::components::icons::Icon;
use leptos::prelude::*;

#[component]
fn ThemeToggle() -> impl IntoView {
    #[allow(unused_variables)]
    let (is_dark, set_is_dark) = signal(false);

    // Initialize theme from localStorage / system preference
    #[cfg(not(feature = "ssr"))]
    {
        Effect::new(move |prev: Option<()>| {
            if prev.is_some() {
                return; // Only run once
            }
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            // Check if dark class is already set (by FOUC prevention script)
            let is_currently_dark = document
                .document_element()
                .map(|el| el.class_list().contains("dark"))
                .unwrap_or(false);
            set_is_dark.set(is_currently_dark);
        });
    }

    #[cfg(not(feature = "ssr"))]
    let toggle_theme = move |_| {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let html = document.document_element().unwrap();
        let new_dark = !is_dark.get_untracked();

        if new_dark {
            html.class_list().add_1("dark").unwrap();
        } else {
            html.class_list().remove_1("dark").unwrap();
        }

        // Persist to localStorage
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.set_item("theme", if new_dark { "dark" } else { "light" });
        }

        set_is_dark.set(new_dark);
    };

    #[cfg(feature = "ssr")]
    let toggle_theme = move |_: leptos::ev::MouseEvent| {};

    view! {
        <button
            class="px-2 py-2 text-charcoal hover:text-accent transition-colors duration-200"
            on:click=toggle_theme
            aria-label="Toggle dark mode"
        >
            <Show
                when=move || is_dark()
                fallback=|| view! { <Icon name="moon" class="w-5 h-5" /> }
            >
                <Icon name="sun" class="w-5 h-5" />
            </Show>
        </button>
    }
}

#[component]
pub fn Nav() -> impl IntoView {
    let routes = vec![
        ("#about", "About", "about"),
        ("#cluster", "Cluster", "cluster"),
        ("#satellites", "Satellites", "satellites"),
        ("#life", "Game of Life", "life"),
        ("#path", "Pathfinding", "path"),
        ("#photography", "Photography", "photography")
    ];
    #[allow(unused_variables)]
    let (active_section, set_active_section) = signal(String::new());

    let contact_links = vec![
        (
            "https://github.com/jaydh",
            "github",
            true,
        ),
        (
            "https://www.linkedin.com/in/jaydanhoward/",
            "linkedin",
            true,
        ),
        (
            "mailto:hello@jaydanhoward.com",
            "envelope",
            true,
        ),
    ];

    let (show_contact_links, set_show_contact_links) = signal(false);

    // Set up scroll spy with scroll event listener (throttled with RAF)
    #[cfg(not(feature = "ssr"))]
    {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;
        use std::rc::Rc;
        use std::cell::RefCell;

        Effect::new(move |_| {
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();

            // Track if we've scheduled an update
            let ticking = Rc::new(RefCell::new(false));

            let update_active_section = move || {
                let document = web_sys::window().unwrap().document().unwrap();

                // Get the scroll container
                if document.get_element_by_id("main-scroll-container").is_some() {
                    let container_top = 100.0; // Account for nav height

                    // Check each section and find which one is most visible
                    let section_ids = vec!["about", "cluster", "satellites", "life", "path", "photography"];
                    let mut best_section = String::new();
                    let mut min_distance = f64::MAX;

                    for section_id in section_ids {
                        if let Some(section) = document.get_element_by_id(section_id) {
                            let rect = section.get_bounding_client_rect();
                            let section_top = rect.top();
                            let section_bottom = rect.bottom();

                            // If section is in viewport (accounting for nav bar)
                            if section_top < container_top + 200.0 && section_bottom > container_top {
                                // Calculate distance from ideal position (just below nav)
                                let distance = (section_top - container_top).abs();
                                if distance < min_distance {
                                    min_distance = distance;
                                    best_section = section_id.to_string();
                                }
                            }
                        }
                    }

                    set_active_section.set(best_section);
                }
            };

            let ticking_clone = ticking.clone();
            let scroll_handler = Closure::wrap(Box::new(move || {
                if !*ticking_clone.borrow() {
                    let window = web_sys::window().unwrap();
                    let ticking_inner = ticking_clone.clone();

                    let callback = Closure::once(move || {
                        update_active_section();
                        *ticking_inner.borrow_mut() = false;
                    });

                    let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
                    callback.forget();

                    *ticking_clone.borrow_mut() = true;
                }
            }) as Box<dyn Fn()>);

            // Attach scroll listener to the scroll container
            if let Some(container) = document.get_element_by_id("main-scroll-container") {
                container
                    .add_event_listener_with_callback("scroll", scroll_handler.as_ref().unchecked_ref())
                    .unwrap();
            }

            scroll_handler.forget();
        });
    }

    view! {
        <nav class="sticky top-0 flex flex-row pointer-events-auto px-8 py-6 text-base border-b border-border bg-surface z-40">
            <div class="flex items-center gap-8 max-w-7xl mx-auto w-full">
                <a
                    href="#about"
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
                        .map(|(route, display_text, section_id)| {
                            let section_id = section_id.to_string();
                            view! {
                                <a
                                    href=route
                                    class=move || {
                                        let base = "px-4 py-2 relative transition-all duration-200";
                                        if active_section() == section_id {
                                            format!("{base} text-accent font-medium after:absolute after:bottom-0 after:left-0 after:right-0 after:h-0.5 after:bg-accent after:scale-x-100 after:transition-transform after:duration-200")
                                        } else {
                                            format!("{base} text-charcoal hover:text-accent after:absolute after:bottom-0 after:left-0 after:right-0 after:h-0.5 after:bg-accent after:scale-x-0 hover:after:scale-x-100 after:transition-transform after:duration-200")
                                        }
                                    }
                                    on:click=move |_| {
                                        set_show_contact_links.set(false);
                                    }
                                >

                                    {display_text}
                                </a>
                            }
                        })
                        .collect_view()}
                    <ThemeToggle />
                    <div class="flex flex-col relative">
                        <button
                            type="button"
                            class="px-4 py-2 text-charcoal hover:text-accent transition-colors duration-200 flex items-center gap-1"
                            on:click=move |_| {
                                set_show_contact_links.set(!show_contact_links());
                            }
                        >

                            Contact
                            <Icon name="caret-down" class="w-3 h-3" />
                        </button>
                        <div class="absolute top-full mt-2 right-0 z-10">
                            <Show when=move || {
                                show_contact_links()
                            }>
                                <div class="bg-surface border border-border rounded-lg shadow-minimal-lg overflow-hidden min-w-[160px]">
                                    {contact_links
                                        .clone()
                                        .into_iter()
                                        .map(|(route, icon_name, external)| {
                                            let target = if external { "_blank" } else { "_self" };
                                            view! {
                                                <a
                                                    href=route
                                                    class="hover:bg-border hover:bg-opacity-50 hover:text-accent flex items-center gap-3 px-4 py-3 transition-colors duration-200"
                                                    target=target
                                                    rel="noreferrer"
                                                >

                                                    <Icon name=icon_name class="w-5 h-5" />
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
