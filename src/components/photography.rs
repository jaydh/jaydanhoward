use crate::components::source_anchor::SourceAnchor;
use leptos::prelude::*;

#[server]
pub async fn fetch_images() -> Result<Vec<String>, ServerFnError<String>> {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct FileItem {
        name: String,
        is_dir: bool,
    }

    // Create a client with a 10-second timeout to prevent hanging requests
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| {
            tracing::error!("Failed to build HTTP client: {}", e);
            ServerFnError::ServerError("Failed to initialize client".to_string())
        })?;

    let response = client
        .get("https://caddy.jaydanhoward.com")
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch files from caddy: {}", e);
            ServerFnError::ServerError("Failed to fetch files".to_string())
        })?;

    let files: Vec<FileItem> = response.json().await.map_err(|e| {
        tracing::error!("Failed to parse JSON response from caddy: {}", e);
        ServerFnError::ServerError("Failed to parse response".to_string())
    })?;

    let mut media_files = Vec::new();

    for file in files {
        // Skip directories
        if file.is_dir {
            continue;
        }

        let name = &file.name;
        let name_lower = name.to_lowercase();

        // Only include full-size images (for srcset base) and videos
        // We'll construct thumb/medium URLs in the component
        let base_name = if name_lower.ends_with("-full.webp") {
            // Strip the -full.webp suffix to get base name
            Some(name.trim_end_matches("-full.webp").trim_end_matches("-full.WEBP"))
        } else if name_lower.ends_with(".mp4") {
            // Videos don't have size variants
            Some(name.as_str())
        } else {
            None
        };

        if let Some(base) = base_name {
            // Validate that the path doesn't contain directory traversal attempts
            if !base.contains("..") && !base.starts_with('/') {
                let full_url = format!("https://caddy.jaydanhoward.com/{}", base);
                media_files.push(full_url);
            }
        }
    }

    tracing::info!(
        "Successfully fetched {} media files from caddy",
        media_files.len()
    );
    Ok(media_files)
}

// Virtualized media item component that only renders when visible
#[component]
fn VirtualizedMediaItem(
    src: String,
    idx: usize,
    on_click: Callback<usize>,
    #[prop(default = false)] is_priority: bool,
) -> impl IntoView {
    let node_ref = NodeRef::<leptos::html::Div>::new();
    let (is_visible, _set_is_visible) = signal(false);
    #[cfg(feature = "hydrate")]
    let (has_been_visible, set_has_been_visible) = signal(false);
    let (is_loaded, set_is_loaded) = signal(false);

    // Set up Intersection Observer on the client side only
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;

        Effect::new(move |_| {
            if let Some(element) = node_ref.get() {
                let set_is_visible = _set_is_visible.clone();
                let set_has_been_visible = set_has_been_visible.clone();

                // Track timeout ID for delayed unmounting
                use std::cell::RefCell;
                use std::rc::Rc;
                let timeout_id: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));
                let timeout_id_clone = timeout_id.clone();

                // Create callback for intersection observer
                let callback = Closure::wrap(Box::new(move |entries: js_sys::Array| {
                    if let Some(entry) = entries
                        .get(0)
                        .dyn_into::<web_sys::IntersectionObserverEntry>()
                        .ok()
                    {
                        let is_intersecting = entry.is_intersecting();
                        set_is_visible.set(is_intersecting);

                        if is_intersecting {
                            // Item is visible - load it and cancel any pending unload
                            set_has_been_visible.set(true);

                            // Clear timeout if exists
                            if let Some(id) = timeout_id_clone.borrow_mut().take() {
                                if let Some(window) = web_sys::window() {
                                    window.clear_timeout_with_handle(id);
                                }
                            }
                        } else {
                            // Item scrolled out of view - schedule unload after 30 seconds
                            let set_has_been_visible = set_has_been_visible.clone();
                            let timeout_id_inner = timeout_id_clone.clone();

                            let unload_callback = Closure::once(Box::new(move || {
                                set_has_been_visible.set(false);
                                *timeout_id_inner.borrow_mut() = None;
                            })
                                as Box<dyn FnOnce()>);

                            if let Some(window) = web_sys::window() {
                                if let Ok(id) = window
                                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                                        unload_callback.as_ref().unchecked_ref(),
                                        30000, // 30 seconds
                                    )
                                {
                                    *timeout_id_clone.borrow_mut() = Some(id);
                                }
                                unload_callback.forget();
                            }
                        }
                    }
                }) as Box<dyn Fn(js_sys::Array)>);

                // Create intersection observer with rootMargin to load images slightly before they're visible
                let options = web_sys::IntersectionObserverInit::new();
                options.set_root_margin("50px");

                if let Ok(observer) = web_sys::IntersectionObserver::new_with_options(
                    callback.as_ref().unchecked_ref(),
                    &options,
                ) {
                    observer.observe(&element);

                    // Leak the callback to keep it alive (it will be cleaned up when the page unloads)
                    // This is necessary because Closure is not Send/Sync
                    callback.forget();
                }
            }
        });
    }

    let src_clone = src.clone();
    let is_video = src.to_lowercase().ends_with(".mp4");

    view! {
        <div
            node_ref=node_ref
            class="group relative overflow-hidden rounded-xl shadow-minimal-lg cursor-pointer transition-all duration-300 hover:scale-[1.05] hover:shadow-2xl hover:ring-2 hover:ring-primary/50"
            style="contain: layout style paint;"
            on:click=move |_| on_click.run(idx)
        >
            <div class="aspect-square overflow-hidden bg-gradient-to-br from-border to-charcoal/5">
                {move || {
                    // Lazy load + delayed unload: load when visible, keep for 30s after scrolling out
                    #[cfg(feature = "ssr")]
                    let should_render = true;

                    #[cfg(feature = "hydrate")]
                    let should_render = has_been_visible.get();

                    if should_render {
                        if is_video {
                            view! {
                                <video
                                    src=src_clone.clone()
                                    muted=true
                                    loop=true
                                    playsinline=true
                                    preload="metadata"
                                    autoplay=is_visible.get()
                                    class="w-full h-full object-cover transition-opacity duration-300"
                                    class:opacity-0=move || !is_loaded.get()
                                    class:opacity-100=move || is_loaded.get()
                                    on:loadeddata=move |_| set_is_loaded.set(true)
                                />
                                // Play icon overlay
                                <div class="absolute inset-0 flex items-center justify-center pointer-events-none">
                                    <div class="w-16 h-16 rounded-full bg-white/90 backdrop-blur-sm flex items-center justify-center shadow-lg">
                                        <svg class="w-8 h-8 text-charcoal ml-1" fill="currentColor" viewBox="0 0 24 24">
                                            <path d="M8 5v14l11-7z"/>
                                        </svg>
                                    </div>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <img
                                    src=format!("{}-medium.webp", src_clone.clone())
                                    srcset=format!(
                                        "{}-thumb.webp 400w, {}-medium.webp 800w, {}-full.webp 1920w",
                                        src_clone.clone(),
                                        src_clone.clone(),
                                        src_clone.clone()
                                    )
                                    sizes="(max-width: 768px) 100vw, (max-width: 1024px) 50vw, 33vw"
                                    alt=src_clone.clone()
                                    loading=if is_priority { "eager" } else { "lazy" }
                                    decoding="async"
                                    fetchpriority=if is_priority { "high" } else { "auto" }
                                    class="w-full h-full object-cover transition-opacity duration-300"
                                    class:opacity-0=move || !is_loaded.get()
                                    class:opacity-100=move || is_loaded.get()
                                    on:load=move |_| set_is_loaded.set(true)
                                />
                            }.into_any()
                        }
                    } else {
                        // Enhanced skeleton loader when not visible
                        view! {
                            <div class="w-full h-full relative overflow-hidden bg-gradient-to-br from-border via-charcoal/5 to-border">
                                <div class="absolute inset-0 -translate-x-full animate-[shimmer_2s_infinite] bg-gradient-to-r from-transparent via-white/10 to-transparent" />
                            </div>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}

#[component]
pub fn Photography() -> impl IntoView {
    // Fetch images from server
    let images_resource = Resource::new(|| (), |_| async { fetch_images().await });

    // State for preview modal
    let (selected_image, set_selected_image) = signal::<Option<usize>>(None);

    view! {
        <SourceAnchor href="#[git]" />
        <div class="max-w-7xl mx-auto px-8 w-full flex flex-col gap-8 items-center">
            <h1 class="text-3xl font-bold text-charcoal">
                "Photography"
            </h1>
            <Suspense fallback=move || view! {
                <div class="w-full flex justify-center items-center py-20">
                    <p class="text-charcoal-light">"Loading images..."</p>
                </div>
            }>
                {move || {
                    images_resource.get().map(|result| {
                        match result {
                            Ok(images) => {
                                let images_for_modal = images.clone();
                                let on_click = Callback::new(move |idx| set_selected_image.set(Some(idx)));

                                view! {
                                    <div class="w-full flex flex-col gap-6">
                                        <div class="w-full grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6" style="contain: layout style paint;">
                                            {images
                                                .iter()
                                                .enumerate()
                                                .map(|(idx, src)| {
                                                    let src = src.clone();
                                                    // Mark first 6 images as priority (2 rows on desktop)
                                                    let is_priority = idx < 6;
                                                    view! {
                                                        <VirtualizedMediaItem
                                                            src=src
                                                            idx=idx
                                                            on_click=on_click
                                                            is_priority=is_priority
                                                        />
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    </div>
                                    // Preview Modal
                                    {move || {
                                        selected_image().and_then(|idx| {
                                            images_for_modal.get(idx).map(|src| {
                                                let total = images_for_modal.len();
                                                let has_prev = idx > 0;
                                                let has_next = idx < total - 1;

                                                let go_prev = move |_| {
                                                    if idx > 0 {
                                                        set_selected_image.set(Some(idx - 1));
                                                    }
                                                };

                                                let go_next = move |_| {
                                                    if idx < total - 1 {
                                                        set_selected_image.set(Some(idx + 1));
                                                    }
                                                };

                                                // Keyboard controls and swipe gestures
                                                #[cfg(feature = "hydrate")]
                                                {
                                                    use wasm_bindgen::closure::Closure;
                                                    use wasm_bindgen::JsCast;
                                                    use std::cell::RefCell;
                                                    use std::rc::Rc;

                                                    Effect::new(move |_| {
                                                        let window = web_sys::window().expect("window");

                                                        // Keyboard controls
                                                        let keyboard_closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
                                                            match event.key().as_str() {
                                                                "Escape" => set_selected_image.set(None),
                                                                "ArrowLeft" if has_prev => set_selected_image.set(Some(idx - 1)),
                                                                "ArrowRight" if has_next => set_selected_image.set(Some(idx + 1)),
                                                                _ => {}
                                                            }
                                                        }) as Box<dyn Fn(web_sys::KeyboardEvent)>);

                                                        let _ = window.add_event_listener_with_callback(
                                                            "keydown",
                                                            keyboard_closure.as_ref().unchecked_ref()
                                                        );

                                                        keyboard_closure.forget();

                                                        // Touch/swipe gestures
                                                        let touch_start_x = Rc::new(RefCell::new(0.0));
                                                        let touch_start_x_clone = touch_start_x.clone();

                                                        let touchstart_closure = Closure::wrap(Box::new(move |event: web_sys::TouchEvent| {
                                                            if let Some(touch) = event.touches().get(0) {
                                                                *touch_start_x_clone.borrow_mut() = touch.client_x() as f64;
                                                            }
                                                        }) as Box<dyn Fn(web_sys::TouchEvent)>);

                                                        let touchend_closure = Closure::wrap(Box::new(move |event: web_sys::TouchEvent| {
                                                            if let Some(touch) = event.changed_touches().get(0) {
                                                                let touch_end_x = touch.client_x() as f64;
                                                                let touch_start = *touch_start_x.borrow();
                                                                let diff = touch_end_x - touch_start;

                                                                // Swipe threshold of 50px
                                                                if diff.abs() > 50.0 {
                                                                    if diff > 0.0 && has_prev {
                                                                        // Swipe right - go to previous
                                                                        set_selected_image.set(Some(idx - 1));
                                                                    } else if diff < 0.0 && has_next {
                                                                        // Swipe left - go to next
                                                                        set_selected_image.set(Some(idx + 1));
                                                                    }
                                                                }
                                                            }
                                                        }) as Box<dyn Fn(web_sys::TouchEvent)>);

                                                        let _ = window.add_event_listener_with_callback(
                                                            "touchstart",
                                                            touchstart_closure.as_ref().unchecked_ref()
                                                        );

                                                        let _ = window.add_event_listener_with_callback(
                                                            "touchend",
                                                            touchend_closure.as_ref().unchecked_ref()
                                                        );

                                                        touchstart_closure.forget();
                                                        touchend_closure.forget();
                                                    });
                                                }

                                                view! {
                                                    <div
                                                        class="fixed inset-0 z-50 flex items-center justify-center bg-charcoal/95 backdrop-blur-md animate-in fade-in duration-300"
                                                        on:click=move |_| set_selected_image.set(None)
                                                    >
                                                        // Close button
                                                        <button
                                                            class="absolute top-4 right-4 text-white/80 hover:text-white text-5xl font-light leading-none z-10 w-14 h-14 flex items-center justify-center rounded-full hover:bg-white/10 transition-all hover:scale-110"
                                                            on:click=move |_| set_selected_image.set(None)
                                                            aria-label="Close preview"
                                                        >
                                                            "×"
                                                        </button>

                                                        // Image counter
                                                        <div class="absolute top-4 left-4 text-white/80 text-sm font-medium bg-charcoal/50 px-3 py-1.5 rounded-full backdrop-blur-sm z-10">
                                                            {format!("{} / {}", idx + 1, total)}
                                                        </div>

                                                        // Previous button
                                                        {has_prev.then(|| view! {
                                                            <button
                                                                class="absolute left-4 top-1/2 -translate-y-1/2 text-white/80 hover:text-white text-5xl font-light z-10 w-14 h-14 flex items-center justify-center rounded-full bg-charcoal/50 hover:bg-charcoal/70 backdrop-blur-sm transition-all hover:scale-110"
                                                                on:click=move |e| { e.stop_propagation(); go_prev(e); }
                                                                aria-label="Previous image"
                                                            >
                                                                "‹"
                                                            </button>
                                                        })}

                                                        // Next button
                                                        {has_next.then(|| view! {
                                                            <button
                                                                class="absolute right-4 top-1/2 -translate-y-1/2 text-white/80 hover:text-white text-5xl font-light z-10 w-14 h-14 flex items-center justify-center rounded-full bg-charcoal/50 hover:bg-charcoal/70 backdrop-blur-sm transition-all hover:scale-110"
                                                                on:click=move |e| { e.stop_propagation(); go_next(e); }
                                                                aria-label="Next image"
                                                            >
                                                                "›"
                                                            </button>
                                                        })}

                                                        <div
                                                            class="relative max-w-7xl max-h-screen p-4 md:p-8 animate-in zoom-in-95 duration-300"
                                                            on:click=move |e| e.stop_propagation()
                                                        >
                                                            {
                                                                if src.to_lowercase().ends_with(".mp4") {
                                                                    view! {
                                                                        <video
                                                                            src=src.clone()
                                                                            controls=true
                                                                            loop=true
                                                                            playsinline=true
                                                                            preload="metadata"
                                                                            autoplay=true
                                                                            class="max-w-full max-h-[90vh] object-contain rounded-lg shadow-2xl"
                                                                        />
                                                                    }.into_any()
                                                                } else {
                                                                    view! {
                                                                        <img
                                                                            src=format!("{}-full.webp", src.clone())
                                                                            alt=src.clone()
                                                                            class="max-w-full max-h-[90vh] object-contain rounded-lg shadow-2xl"
                                                                        />
                                                                    }.into_any()
                                                                }
                                                            }
                                                        </div>
                                                    </div>
                                                }
                                            })
                                        })
                                    }}
                                }.into_any()
                            },
                            Err(_) => view! {
                                <div class="w-full flex justify-center items-center py-20">
                                    <p class="text-red-500">"Unable to load images. Please try again later."</p>
                                </div>
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}
