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
        .get("https://caddy.jaydanhoward.com/data/")
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch files from caddy: {}", e);
            ServerFnError::ServerError("Failed to fetch files".to_string())
        })?;

    let files: Vec<FileItem> = response.json()
        .await
        .map_err(|e| {
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

        // Only include image and video files (case-insensitive)
        if name_lower.ends_with(".webp") || name_lower.ends_with(".jpg") || name_lower.ends_with(".png") || name_lower.ends_with(".mp4") {
            // Validate that the path doesn't contain directory traversal attempts
            if !name.contains("..") && !name.starts_with('/') {
                let full_url = format!("https://caddy.jaydanhoward.com/data/{}", name);
                media_files.push(full_url);
            }
        }
    }

    tracing::info!("Successfully fetched {} media files from caddy", media_files.len());
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
    let (is_loaded, set_is_loaded) = signal(false);

    // Set up Intersection Observer on the client side only
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;

        Effect::new(move |_| {
            if let Some(element) = node_ref.get() {
                let set_is_visible = _set_is_visible.clone();

                // Create callback for intersection observer
                let callback = Closure::wrap(Box::new(move |entries: js_sys::Array| {
                    if let Some(entry) = entries.get(0).dyn_into::<web_sys::IntersectionObserverEntry>().ok() {
                        set_is_visible.set(entry.is_intersecting());
                    }
                }) as Box<dyn Fn(js_sys::Array)>);

                // Create intersection observer with rootMargin to load images slightly before they're visible
                let mut options = web_sys::IntersectionObserverInit::new();
                options.root_margin("50px");

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
            class="group relative overflow-hidden rounded-xl shadow-minimal-lg cursor-pointer transition-transform hover:scale-[1.02]"
            style="contain: layout style paint;"
            on:click=move |_| on_click.run(idx)
        >
            <div class="aspect-square overflow-hidden bg-gradient-to-br from-border to-charcoal/5">
                {move || {
                    // Only render media when visible (or always on server for SSR)
                    #[cfg(feature = "ssr")]
                    let should_render = true;

                    #[cfg(feature = "hydrate")]
                    let should_render = is_visible.get();

                    if should_render {
                        if is_video {
                            view! {
                                <video
                                    src=src_clone.clone()
                                    muted=true
                                    loop=true
                                    playsinline=true
                                    autoplay=is_visible.get()
                                    class="w-full h-full object-cover transition-opacity duration-300"
                                    class:opacity-0=move || !is_loaded.get()
                                    class:opacity-100=move || is_loaded.get()
                                    on:loadeddata=move |_| set_is_loaded.set(true)
                                />
                            }.into_any()
                        } else {
                            view! {
                                <img
                                    src=src_clone.clone()
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
                                                view! {
                                                    <div
                                                        class="fixed inset-0 z-50 flex items-center justify-center bg-charcoal/90 backdrop-blur-sm animate-in fade-in duration-200"
                                                        on:click=move |_| set_selected_image(None)
                                                    >
                                                        <div
                                                            class="relative max-w-7xl max-h-screen p-4 md:p-8"
                                                            on:click=move |e| e.stop_propagation()
                                                        >
                                                            <button
                                                                class="absolute top-2 right-2 md:top-4 md:right-4 text-white/90 hover:text-white text-4xl font-light leading-none z-10 w-12 h-12 flex items-center justify-center rounded-full hover:bg-white/10 transition-all"
                                                                on:click=move |_| set_selected_image(None)
                                                                aria-label="Close preview"
                                                            >
                                                                "×"
                                                            </button>
                                                            {
                                                                if src.to_lowercase().ends_with(".mp4") {
                                                                    view! {
                                                                        <video
                                                                            src=src.clone()
                                                                            controls=true
                                                                            loop=true
                                                                            playsinline=true
                                                                            autoplay=true
                                                                            class="max-w-full max-h-[90vh] object-contain rounded-lg shadow-2xl"
                                                                        />
                                                                    }.into_any()
                                                                } else {
                                                                    view! {
                                                                        <img
                                                                            src=src.clone()
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
