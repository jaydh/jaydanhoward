use crate::components::source_anchor::SourceAnchor;
use leptos::prelude::*;

#[server]
pub async fn fetch_images() -> Result<Vec<String>, ServerFnError<String>> {
    use scraper::{Html, Selector};

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
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch images from caddy: {}", e);
            ServerFnError::ServerError("Failed to fetch images".to_string())
        })?;

    let html_content = response.text()
        .await
        .map_err(|e| {
            tracing::error!("Failed to parse response from caddy: {}", e);
            ServerFnError::ServerError("Failed to parse response".to_string())
        })?;

    // Parse HTML using scraper for safe, robust parsing
    let document = Html::parse_document(&html_content);

    // Select all anchor tags
    let selector = Selector::parse("a").map_err(|e| {
        tracing::error!("Failed to create CSS selector: {}", e);
        ServerFnError::ServerError("Failed to parse HTML".to_string())
    })?;

    let mut images = Vec::new();

    for element in document.select(&selector) {
        if let Some(href) = element.value().attr("href") {
            // Only include image and video files
            if href.ends_with(".webp") || href.ends_with(".jpg") || href.ends_with(".png") || href.ends_with(".mp4") {
                // Strip leading ./ if present
                let clean_path = href.strip_prefix("./").unwrap_or(href);

                // Validate that the path doesn't contain directory traversal attempts
                if !clean_path.contains("..") && !clean_path.starts_with('/') {
                    let full_url = format!("https://caddy.jaydanhoward.com/data/{}", clean_path);
                    images.push(full_url);
                }
            }
        }
    }

    tracing::info!("Successfully fetched {} images from caddy", images.len());
    Ok(images)
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
                            Ok(images) => view! {
                                    <div class="w-full flex flex-col gap-6">
                                        <div class="w-full grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6" style="contain: layout style paint;">
                                            {images
                                                .iter()
                                                .enumerate()
                                                .map(|(idx, src)| {
                                                    let src = src.clone();
                                                    view! {
                                                        <div
                                                            class="group relative overflow-hidden rounded-xl shadow-minimal-lg cursor-pointer transition-transform hover:scale-[1.02]"
                                                            style="contain: layout style paint;"
                                                            on:click=move |_| set_selected_image(Some(idx))
                                                        >
                                                            <div class="aspect-square overflow-hidden bg-border">
                                                                {
                                                                    if src.ends_with(".mp4") {
                                                                        view! {
                                                                            <video
                                                                                src=src.clone()
                                                                                muted=true
                                                                                loop=true
                                                                                playsinline=true
                                                                                autoplay=true
                                                                                class="w-full h-full object-cover"
                                                                            />
                                                                        }.into_any()
                                                                    } else {
                                                                        view! {
                                                                            <img
                                                                                src=src.clone()
                                                                                alt=src.clone()
                                                                                loading="lazy"
                                                                                decoding="async"
                                                                                class="w-full h-full object-cover"
                                                                            />
                                                                        }.into_any()
                                                                    }
                                                                }
                                                            </div>
                                                        </div>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    </div>
                                // Preview Modal
                                {move || {
                                    images_resource.get().and_then(|result| {
                                        result.ok().and_then(|images| {
                                            selected_image().map(|idx| {
                                                let src = &images[idx];
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
                                                                if src.ends_with(".mp4") {
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
                                    })
                                }}
                            }.into_any(),
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
