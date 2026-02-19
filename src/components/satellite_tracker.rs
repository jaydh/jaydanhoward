#![allow(clippy::all)]
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TleData {
    pub name: String,
    pub line1: String,
    pub line2: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SatelliteGroup {
    pub group_name: String,
    pub satellites: Vec<TleData>,
}

#[server(name = GetTleData, prefix = "/api", endpoint = "get_tle_data")]
pub async fn get_tle_data(group: String) -> Result<Vec<TleData>, ServerFnError<String>> {
    // CelesTrak provides various satellite groups
    // Popular groups: "starlink", "active", "stations", "visual", "gps-ops"
    let url = match group.as_str() {
        "active" => "https://celestrak.org/NORAD/elements/gp.php?GROUP=active&FORMAT=tle",
        "starlink" => "https://celestrak.org/NORAD/elements/gp.php?GROUP=starlink&FORMAT=tle",
        "stations" => "https://celestrak.org/NORAD/elements/gp.php?GROUP=stations&FORMAT=tle",
        "visual" => "https://celestrak.org/NORAD/elements/gp.php?GROUP=visual&FORMAT=tle",
        "gps-ops" => "https://celestrak.org/NORAD/elements/gp.php?GROUP=gps-ops&FORMAT=tle",
        _ => return Err(ServerFnError::ServerError(format!("Unknown group: {}", group))),
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| ServerFnError::ServerError(format!("Failed to create HTTP client: {}", e)))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| ServerFnError::ServerError(format!("Failed to fetch TLE data: {}", e)))?;

    if !response.status().is_success() {
        return Err(ServerFnError::ServerError(format!(
            "CelesTrak returned HTTP {}",
            response.status()
        )));
    }

    let text = response
        .text()
        .await
        .map_err(|e| ServerFnError::ServerError(format!("Failed to read response: {}", e)))?;

    // Parse TLE format (3 lines per satellite: name, line1, line2)
    // Filter blank lines first to avoid misalignment from leading/trailing whitespace or
    // any extra lines in the response.
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut satellites = Vec::new();

    for chunk in lines.chunks(3) {
        if chunk.len() == 3
            && chunk[1].trim_start().starts_with("1 ")
            && chunk[2].trim_start().starts_with("2 ")
        {
            satellites.push(TleData {
                name: chunk[0].trim().to_string(),
                line1: chunk[1].trim().to_string(),
                line2: chunk[2].trim().to_string(),
            });
        }
    }

    Ok(satellites)
}

#[component]
pub fn SatelliteTracker() -> impl IntoView {
    let (tle_data, set_tle_data) = signal(Vec::<TleData>::new());
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(None::<String>);

    // Canvas reference for WebGL rendering
    let canvas_ref = NodeRef::<leptos::tachys::html::element::Canvas>::new();

    // Signal for displaying current simulation date
    #[cfg_attr(feature = "ssr", allow(unused_variables))]
    let (current_date_display, set_current_date_display) = signal(String::new());

    // Renderer value for zoom controls (needs to be accessible outside Effect)
    #[cfg(not(feature = "ssr"))]
    let (renderer_signal, set_renderer_signal) = signal(None::<StoredValue<crate::components::satellite_renderer::SatelliteRenderer, leptos::prelude::LocalStorage>>);

    // Visibility tracking for intersection observer
    #[cfg(not(feature = "ssr"))]
    let (is_visible, set_is_visible) = signal(false);

    // Zoom button long press handling
    #[cfg(not(feature = "ssr"))]
    use std::rc::Rc;
    #[cfg(not(feature = "ssr"))]
    use std::cell::RefCell;
    #[cfg(not(feature = "ssr"))]
    let zoom_interval: Rc<RefCell<Option<leptos::leptos_dom::helpers::IntervalHandle>>> = Rc::new(RefCell::new(None));

    // Animation speed control (frames to skip: 0 = every frame, 1 = every other frame, etc.)
    #[cfg(not(feature = "ssr"))]
    let (frame_skip, set_frame_skip) = signal(2_usize); // Default: advance every 2 frames

    // Fetch TLE data when component mounts
    Effect::new(move |_| {
        set_loading.set(true);
        set_error.set(None);

        leptos::task::spawn_local(async move {
            match get_tle_data("active".to_string()).await {
                Ok(data) => {
                    #[cfg(not(feature = "ssr"))]
                    {
                        web_sys::console::log_1(&format!("Loaded {} TLE entries", data.len()).into());

                        // Parse TLE data into satellite objects
                        use crate::components::satellite_calculations;
                        let tle_tuples: Vec<(String, String, String)> = data
                            .iter()
                            .map(|tle| (tle.name.clone(), tle.line1.clone(), tle.line2.clone()))
                            .collect();

                        let satellites = satellite_calculations::parse_satellites(&tle_tuples);
                        web_sys::console::log_1(&format!("Parsed {} satellites", satellites.len()).into());

                        // Store satellites in StoredValue for the render loop to access
                        // This will be handled by the renderer initialization
                    }
                    set_tle_data.set(data);
                    set_error.set(None);
                }
                Err(e) => {
                    #[cfg(feature = "ssr")]
                    tracing::error!("Failed to fetch TLE data: {:?}", e);
                    #[cfg(not(feature = "ssr"))]
                    web_sys::console::error_1(&format!("Failed to fetch TLE data: {:?}", e).into());
                    set_error.set(Some(format!("Failed to load satellites: {:?}", e)));
                }
            }
            set_loading.set(false);
        });
    });

    // Initialize WebGL context and rendering (client-side only)
    #[cfg(not(feature = "ssr"))]
    Effect::new(move |_| {
        use crate::components::satellite_renderer::SatelliteRenderer;
        use crate::components::satellite_calculations;
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;

        if let Some(canvas) = canvas_ref.get_untracked() {
            web_sys::console::log_1(&"Canvas ready for WebGL initialization".into());

            // Get WebGL2 context
            let gl_context = canvas
                .get_context("webgl2")
                .ok()
                .flatten()
                .and_then(|ctx| ctx.dyn_into::<web_sys::WebGl2RenderingContext>().ok());

            if let Some(gl) = gl_context {
                web_sys::console::log_1(&"WebGL2 context created successfully".into());

                // Initialize renderer
                match SatelliteRenderer::new(gl.clone()) {
                    Ok(mut renderer) => {
                        if let Err(e) = renderer.initialize() {
                            web_sys::console::error_1(&format!("Failed to initialize renderer: {}", e).into());
                            set_error.set(Some(format!("Failed to initialize renderer: {}", e)));
                            return;
                        }

                        // Store renderer in a StoredValue for the render loop
                        let renderer_value = StoredValue::new_local(renderer);

                        // Make renderer accessible for zoom buttons
                        set_renderer_signal.set(Some(renderer_value));

                        // Add mouse drag controls
                        let is_dragging = StoredValue::new_local(false);
                        let last_mouse_x = StoredValue::new_local(0.0_f64);
                        let last_mouse_y = StoredValue::new_local(0.0_f64);

                        let canvas_element = canvas.clone();

                        // Mouse down
                        let mousedown_callback = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
                            is_dragging.set_value(true);
                            last_mouse_x.set_value(e.client_x() as f64);
                            last_mouse_y.set_value(e.client_y() as f64);
                        }) as Box<dyn FnMut(_)>);

                        canvas_element
                            .add_event_listener_with_callback("mousedown", mousedown_callback.as_ref().unchecked_ref())
                            .unwrap();
                        mousedown_callback.forget();

                        // Mouse move
                        let canvas_element2 = canvas.clone();
                        let mousemove_callback = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
                            if is_dragging.get_value() {
                                let current_x = e.client_x() as f64;
                                let current_y = e.client_y() as f64;
                                let delta_x = (current_x - last_mouse_x.get_value()) as f32;
                                let delta_y = (current_y - last_mouse_y.get_value()) as f32;

                                renderer_value.update_value(|renderer| {
                                    renderer.rotate_camera(delta_x, delta_y);
                                });

                                last_mouse_x.set_value(current_x);
                                last_mouse_y.set_value(current_y);
                            }
                        }) as Box<dyn FnMut(_)>);

                        canvas_element2
                            .add_event_listener_with_callback("mousemove", mousemove_callback.as_ref().unchecked_ref())
                            .unwrap();
                        mousemove_callback.forget();

                        // Mouse up
                        let canvas_element3 = canvas.clone();
                        let mouseup_callback = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
                            is_dragging.set_value(false);
                        }) as Box<dyn FnMut(_)>);

                        canvas_element3
                            .add_event_listener_with_callback("mouseup", mouseup_callback.as_ref().unchecked_ref())
                            .unwrap();
                        mouseup_callback.forget();

                        // Mouse leave (stop dragging if mouse leaves canvas)
                        let mouseleave_callback = Closure::wrap(Box::new(move |_: web_sys::MouseEvent| {
                            is_dragging.set_value(false);
                        }) as Box<dyn FnMut(_)>);

                        canvas
                            .add_event_listener_with_callback("mouseleave", mouseleave_callback.as_ref().unchecked_ref())
                            .unwrap();
                        mouseleave_callback.forget();

                        // Store satellites for the render loop
                        let satellites_value = StoredValue::new_local(Vec::<satellite_calculations::Satellite>::new());

                        // Generate 24 hours of timestamps (every 5 minutes = 288 time points)
                        let now_ms = js_sys::Date::now();
                        let twenty_four_hours_ms = 24.0 * 60.0 * 60.0 * 1000.0;
                        let start_time = now_ms - twenty_four_hours_ms;
                        let time_step = 5.0 * 60.0 * 1000.0; // 5 minutes in ms
                        let num_steps = (twenty_four_hours_ms / time_step) as usize;

                        let time_points: Vec<f64> = (0..num_steps)
                            .map(|i| start_time + (i as f64 * time_step))
                            .collect();

                        let time_points_value = StoredValue::new_local(time_points);
                        let current_time_index = StoredValue::new_local(0_usize);
                        let frame_counter = StoredValue::new_local(0_usize);

                        // Watch for TLE data changes and update satellites
                        let tle_data_copy = tle_data;
                        Effect::new(move |_| {
                            let data = tle_data_copy.get();
                            if !data.is_empty() {
                                let tle_tuples: Vec<(String, String, String)> = data
                                    .iter()
                                    .map(|tle| (tle.name.clone(), tle.line1.clone(), tle.line2.clone()))
                                    .collect();

                                let sats = satellite_calculations::parse_satellites(&tle_tuples);
                                web_sys::console::log_1(&format!("Updated {} satellites in renderer", sats.len()).into());
                                satellites_value.set_value(sats);
                            }
                        });

                        // Create render loop with requestAnimationFrame
                        let f: std::rc::Rc<std::cell::RefCell<Option<Closure<dyn FnMut()>>>> =
                            std::rc::Rc::new(std::cell::RefCell::new(None));
                        let g = f.clone();

                        *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
                            // Only animate if visible
                            if is_visible.get_untracked() {
                                // Calculate satellite positions and render frame
                                satellites_value.with_value(|satellites| {
                                    if !satellites.is_empty() {
                                        // Get current time point from the loop
                                        let time_index = current_time_index.get_value();
                                        let time_points = time_points_value.get_value();

                                        if time_index < time_points.len() {
                                            let current_time = time_points[time_index];

                                            // Calculate positions at this specific time
                                            let positions = satellite_calculations::calculate_positions_at_time(satellites, current_time);
                                            let sat_positions: Vec<satellite_calculations::SatellitePosition> =
                                                positions.into_iter().map(|(_, pos)| pos).collect();

                                            renderer_value.update_value(|renderer| {
                                                renderer.update_satellites(sat_positions);
                                                renderer.render();
                                            });

                                            // Update date display
                                            use wasm_bindgen::JsValue;
                                            let date = js_sys::Date::new(&JsValue::from_f64(current_time));
                                            let date_str = date.to_iso_string().as_string().unwrap_or_default();
                                            // Format as HH:MM (just time since it's within 24 hours)
                                            if date_str.len() >= 16 {
                                                let formatted = format!("{}", &date_str[11..16]);
                                                set_current_date_display.set(formatted);
                                            }

                                            // Advance to next time point based on speed setting
                                            let frame_count = frame_counter.get_value();
                                            frame_counter.set_value(frame_count + 1);

                                            let skip = frame_skip.get_untracked();
                                            if frame_count % (skip + 1) == 0 {
                                                // Move to next time point and loop back to start
                                                let next_index = (time_index + 1) % time_points.len();
                                                current_time_index.set_value(next_index);
                                            }
                                        }
                                    } else {
                                        renderer_value.update_value(|renderer| {
                                            renderer.render();
                                        });
                                    }
                                });
                            }

                            // Request next frame
                            let window = web_sys::window().unwrap();
                            window
                                .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
                                .unwrap();
                        }) as Box<dyn FnMut()>));

                        // Start the render loop
                        let window = web_sys::window().unwrap();
                        window
                            .request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())
                            .unwrap();

                        web_sys::console::log_1(&"Render loop started".into());
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("Failed to create renderer: {}", e).into());
                        set_error.set(Some(format!("Failed to create renderer: {}", e)));
                    }
                }
            } else {
                web_sys::console::error_1(&"Failed to get WebGL2 context".into());
                set_error.set(Some("WebGL2 not supported in this browser".to_string()));
            }
        }
    });

    // Set up intersection observer to pause when not visible
    #[cfg(not(feature = "ssr"))]
    Effect::new(move |_| {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;

        if let Some(canvas) = canvas_ref.get_untracked() {
            let callback = Closure::wrap(Box::new(move |entries: js_sys::Array| {
                if let Some(entry) = entries.get(0).dyn_into::<web_sys::IntersectionObserverEntry>().ok() {
                    set_is_visible.set(entry.is_intersecting());
                }
            }) as Box<dyn FnMut(js_sys::Array)>);

            let options = web_sys::IntersectionObserverInit::new();
            options.set_threshold(&JsValue::from_f64(0.1)); // Trigger when 10% visible

            if let Ok(observer) = web_sys::IntersectionObserver::new_with_options(
                callback.as_ref().unchecked_ref(),
                &options
            ) {
                observer.observe(&canvas);
                callback.forget();
            }
        }
    });

    view! {
        <div class="w-full bg-gradient-to-br from-gray-900 to-black py-6 px-4 rounded-xl mb-8">

            {move || {
                error.get().map(|err| view! {
                    <div class="text-center text-red-500 p-4 mb-4 bg-red-500/10 rounded">
                        <p>{err}</p>
                    </div>
                })
            }}

            {move || {
                loading.get().then(|| view! {
                    <div class="text-center text-white p-4">
                        <p>"Loading satellite data..."</p>
                    </div>
                })
            }}

            <div class="relative w-full" style="height: 600px;">
                <canvas
                    node_ref=canvas_ref
                    class="w-full h-full rounded-lg"
                    width="1200"
                    height="600"
                >
                    "Your browser does not support canvas"
                </canvas>

                // Zoom controls
                <div class="absolute top-4 right-4 flex flex-col gap-2">
                    {
                        #[cfg(not(feature = "ssr"))]
                        {
                            let zoom_interval_in = zoom_interval.clone();
                            let zoom_interval_up1 = zoom_interval.clone();
                            let zoom_interval_leave1 = zoom_interval.clone();
                            let zoom_interval_out = zoom_interval.clone();
                            let zoom_interval_up2 = zoom_interval.clone();
                            let zoom_interval_leave2 = zoom_interval.clone();

                            view! {
                                <button
                                    class="bg-black/70 hover:bg-black/90 text-white px-3 py-2 rounded text-lg font-bold select-none"
                                    on:click=move |_| {
                                        if let Some(renderer_value) = renderer_signal.get() {
                                            renderer_value.update_value(|renderer| {
                                                renderer.adjust_zoom(1.0);
                                            });
                                        }
                                    }
                                    on:mousedown=move |_| {
                                        let zoom_interval = zoom_interval_in.clone();
                                        if let Some(handle) = zoom_interval.borrow_mut().take() {
                                            handle.clear();
                                        }
                                        leptos::leptos_dom::helpers::set_timeout(
                                            move || {
                                                let zoom_interval = zoom_interval.clone();
                                                let interval_handle = leptos::leptos_dom::helpers::set_interval_with_handle(
                                                    move || {
                                                        if let Some(renderer_value) = renderer_signal.get() {
                                                            renderer_value.update_value(|renderer| {
                                                                renderer.adjust_zoom(1.0);
                                                            });
                                                        }
                                                    },
                                                    std::time::Duration::from_millis(50),
                                                ).ok();
                                                *zoom_interval.borrow_mut() = interval_handle;
                                            },
                                            std::time::Duration::from_millis(200),
                                        );
                                    }
                                    on:mouseup=move |_| {
                                        if let Some(handle) = zoom_interval_up1.borrow_mut().take() {
                                            handle.clear();
                                        }
                                    }
                                    on:mouseleave=move |_| {
                                        if let Some(handle) = zoom_interval_leave1.borrow_mut().take() {
                                            handle.clear();
                                        }
                                    }
                                >
                                    "+"
                                </button>
                                <button
                                    class="bg-black/70 hover:bg-black/90 text-white px-3 py-2 rounded text-lg font-bold select-none"
                                    on:click=move |_| {
                                        if let Some(renderer_value) = renderer_signal.get() {
                                            renderer_value.update_value(|renderer| {
                                                renderer.adjust_zoom(-1.0);
                                            });
                                        }
                                    }
                                    on:mousedown=move |_| {
                                        let zoom_interval = zoom_interval_out.clone();
                                        if let Some(handle) = zoom_interval.borrow_mut().take() {
                                            handle.clear();
                                        }
                                        leptos::leptos_dom::helpers::set_timeout(
                                            move || {
                                                let zoom_interval = zoom_interval.clone();
                                                let interval_handle = leptos::leptos_dom::helpers::set_interval_with_handle(
                                                    move || {
                                                        if let Some(renderer_value) = renderer_signal.get() {
                                                            renderer_value.update_value(|renderer| {
                                                                renderer.adjust_zoom(-1.0);
                                                            });
                                                        }
                                                    },
                                                    std::time::Duration::from_millis(50),
                                                ).ok();
                                                *zoom_interval.borrow_mut() = interval_handle;
                                            },
                                            std::time::Duration::from_millis(200),
                                        );
                                    }
                                    on:mouseup=move |_| {
                                        if let Some(handle) = zoom_interval_up2.borrow_mut().take() {
                                            handle.clear();
                                        }
                                    }
                                    on:mouseleave=move |_| {
                                        if let Some(handle) = zoom_interval_leave2.borrow_mut().take() {
                                            handle.clear();
                                        }
                                    }
                                >
                                    "-"
                                </button>
                            }
                        }
                        #[cfg(feature = "ssr")]
                        {
                            view! {
                                <button class="bg-black/70 hover:bg-black/90 text-white px-3 py-2 rounded text-lg font-bold select-none">
                                    "+"
                                </button>
                                <button class="bg-black/70 hover:bg-black/90 text-white px-3 py-2 rounded text-lg font-bold select-none">
                                    "-"
                                </button>
                            }
                        }
                    }
                </div>

                // View preset controls
                <div class="absolute top-4 left-4 flex flex-col gap-2">
                    <button
                        class="bg-black/70 hover:bg-black/90 text-white px-3 py-1 rounded text-sm"
                        on:click=move |_| {
                            #[cfg(not(feature = "ssr"))]
                            {
                                if let Some(renderer_value) = renderer_signal.get() {
                                    renderer_value.update_value(|renderer| {
                                        renderer.set_preset_view("equator");
                                    });
                                }
                            }
                        }
                    >
                        "Equator"
                    </button>
                    <button
                        class="bg-black/70 hover:bg-black/90 text-white px-3 py-1 rounded text-sm"
                        on:click=move |_| {
                            #[cfg(not(feature = "ssr"))]
                            {
                                if let Some(renderer_value) = renderer_signal.get() {
                                    renderer_value.update_value(|renderer| {
                                        renderer.set_preset_view("north");
                                    });
                                }
                            }
                        }
                    >
                        "North"
                    </button>
                    <button
                        class="bg-black/70 hover:bg-black/90 text-white px-3 py-1 rounded text-sm"
                        on:click=move |_| {
                            #[cfg(not(feature = "ssr"))]
                            {
                                if let Some(renderer_value) = renderer_signal.get() {
                                    renderer_value.update_value(|renderer| {
                                        renderer.set_preset_view("south");
                                    });
                                }
                            }
                        }
                    >
                        "South"
                    </button>
                    <button
                        class="bg-black/70 hover:bg-black/90 text-white px-3 py-1 rounded text-sm"
                        on:click=move |_| {
                            #[cfg(not(feature = "ssr"))]
                            {
                                if let Some(renderer_value) = renderer_signal.get() {
                                    renderer_value.update_value(|renderer| {
                                        renderer.set_preset_view("oblique");
                                    });
                                }
                            }
                        }
                    >
                        "Oblique"
                    </button>
                </div>

                <div class="absolute bottom-4 left-4 bg-black/70 text-white px-3 py-2 rounded text-sm space-y-2">
                    <div>
                        {move || {
                            let count = tle_data.get().len();
                            format!("Tracking {} satellites", count)
                        }}
                    </div>
                    <div class="text-xs text-gray-300">
                        {move || {
                            let time = current_date_display.get();
                            if !time.is_empty() {
                                format!("Time: {} UTC", time)
                            } else {
                                "Loading timeline...".to_string()
                            }
                        }}
                    </div>
                    {
                        #[cfg(not(feature = "ssr"))]
                        {
                            view! {
                                <div class="flex items-center gap-1.5">
                                    <button
                                        class="bg-white/20 hover:bg-white/30 px-2 py-0.5 rounded text-xs"
                                        on:click=move |_| {
                                            set_frame_skip.update(|skip| {
                                                *skip = (*skip + 1).min(10); // Max: every 11 frames
                                            });
                                        }
                                    >
                                        "-"
                                    </button>
                                    <button
                                        class="bg-white/20 hover:bg-white/30 px-2 py-0.5 rounded text-xs"
                                        on:click=move |_| {
                                            set_frame_skip.update(|skip| {
                                                *skip = skip.saturating_sub(1); // Min: every frame
                                            });
                                        }
                                    >
                                        "+"
                                    </button>
                                </div>
                            }
                        }
                        #[cfg(feature = "ssr")]
                        {
                            view! {
                                <div class="flex items-center gap-1.5">
                                    <button class="bg-white/20 px-2 py-0.5 rounded text-xs">"-"</button>
                                    <button class="bg-white/20 px-2 py-0.5 rounded text-xs">"+"</button>
                                </div>
                            }
                        }
                    }
                </div>

                // Altitude legend
                <div class="absolute bottom-4 right-4 bg-black/80 text-white px-3 py-2 rounded text-xs">
                    <div class="font-semibold mb-1.5">"Orbital Classification"</div>
                    <div class="flex flex-col gap-1">
                        <div class="flex items-center gap-2">
                            <div class="w-3 h-3 rounded-full" style="background-color: rgb(77, 204, 255);"></div>
                            <span>"LEO (< 600km)"</span>
                        </div>
                        <div class="flex items-center gap-2">
                            <div class="w-3 h-3 rounded-full" style="background-color: rgb(128, 255, 128);"></div>
                            <span>"LEO High (600-2000km)"</span>
                        </div>
                        <div class="flex items-center gap-2">
                            <div class="w-3 h-3 rounded-full" style="background-color: rgb(255, 204, 51);"></div>
                            <span>"MEO (2000-20000km)"</span>
                        </div>
                        <div class="flex items-center gap-2">
                            <div class="w-3 h-3 rounded-full" style="background-color: rgb(255, 128, 51);"></div>
                            <span>"MEO High (20000-30000km)"</span>
                        </div>
                        <div class="flex items-center gap-2">
                            <div class="w-3 h-3 rounded-full" style="background-color: rgb(255, 77, 77);"></div>
                            <span>"GEO (35786km, equator)"</span>
                        </div>
                        <div class="flex items-center gap-2">
                            <div class="w-3 h-3 rounded-full" style="background-color: rgb(204, 153, 255);"></div>
                            <span>"HEO (high orbit)"</span>
                        </div>
                    </div>
                </div>
            </div>

            <div class="mt-4 text-xs text-gray-400">
                <p>"Data from CelesTrak • Animating last 24 hours of orbital data"</p>
            </div>
        </div>
    }
}
