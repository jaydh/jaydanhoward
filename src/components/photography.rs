use crate::components::source_anchor::SourceAnchor;
use leptos::prelude::*;

#[server]
pub async fn fetch_images() -> Result<Vec<String>, ServerFnError<String>> {
    let response = reqwest::get("https://caddy.jaydanhoward.com/data/")
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    let html = response.text()
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    // Parse HTML to extract image URLs
    // Caddy directory listing shows links like: <a href="./filename.webp">
    let mut images = Vec::new();
    for line in html.lines() {
        if line.contains("href=\"") && (line.contains(".webp") || line.contains(".jpg") || line.contains(".png")) {
            if let Some(start) = line.find("href=\"") {
                let start = start + 6; // length of "href=\""
                if let Some(end) = line[start..].find("\"") {
                    let path = &line[start..start + end];
                    // Only include image files
                    if path.ends_with(".webp") || path.ends_with(".jpg") || path.ends_with(".png") {
                        // Strip leading ./ if present
                        let clean_path = if path.starts_with("./") {
                            path.strip_prefix("./").unwrap()
                        } else {
                            path
                        };
                        let full_url = format!("https://caddy.jaydanhoward.com/data/{}", clean_path);
                        images.push(full_url);
                    }
                }
            }
        }
    }

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
                    <p class="text-charcoal opacity-75">"Loading images..."</p>
                </div>
            }>
                {move || {
                    images_resource.get().map(|result| {
                        match result {
                            Ok(images) => view! {
                                <div class="w-full grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6" style="contain: layout style paint;">
                                    {images
                                        .iter()
                                        .enumerate()
                                        .map(|(idx, src)| {
                                            let src = src.clone();
                                            view! {
                                                <div
                                                    class="group relative overflow-hidden rounded-xl shadow-minimal-lg cursor-pointer"
                                                    style="contain: layout style paint;"
                                                    on:click=move |_| set_selected_image(Some(idx))
                                                >
                                                    <div class="aspect-square overflow-hidden bg-border">
                                                        <img
                                                            src=src.clone()
                                                            alt=src.clone()
                                                            loading="lazy"
                                                            decoding="async"
                                                            class="w-full h-full object-cover"
                                                        />
                                                    </div>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
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
                                                        <div class="relative max-w-7xl max-h-screen p-4 md:p-8">
                                                            <button
                                                                class="absolute top-2 right-2 md:top-4 md:right-4 text-white/80 hover:text-white text-4xl font-light leading-none z-10 w-12 h-12 flex items-center justify-center rounded-full hover:bg-white/10 transition-all"
                                                                on:click=move |e| {
                                                                    e.stop_propagation();
                                                                    set_selected_image(None);
                                                                }
                                                            >
                                                                "×"
                                                            </button>
                                                            <img
                                                                src=src.clone()
                                                                alt=src.clone()
                                                                class="max-w-full max-h-[90vh] object-contain rounded-lg shadow-2xl"
                                                                on:click=move |e| e.stop_propagation()
                                                            />
                                                        </div>
                                                    </div>
                                                }
                                            })
                                        })
                                    })
                                }}
                            }.into_any(),
                            Err(e) => view! {
                                <div class="w-full flex justify-center items-center py-20">
                                    <p class="text-red-500">"Error loading images: " {e.to_string()}</p>
                                </div>
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}
