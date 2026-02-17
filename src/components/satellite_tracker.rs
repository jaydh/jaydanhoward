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

    let text = response
        .text()
        .await
        .map_err(|e| ServerFnError::ServerError(format!("Failed to read response: {}", e)))?;

    // Parse TLE format (3 lines per satellite: name, line1, line2)
    let lines: Vec<&str> = text.lines().collect();
    let mut satellites = Vec::new();

    for chunk in lines.chunks(3) {
        if chunk.len() == 3 {
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
    let (selected_group, set_selected_group) = signal("starlink".to_string());

    // Canvas reference for WebGL rendering
    let canvas_ref = NodeRef::<leptos::tachys::html::element::Canvas>::new();

    // Fetch TLE data when component mounts or group changes
    Effect::new(move |_| {
        let group = selected_group.get();
        set_loading.set(true);
        set_error.set(None);

        leptos::task::spawn_local(async move {
            match get_tle_data(group).await {
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

                        // Store satellites for the render loop
                        let satellites_value = StoredValue::new_local(Vec::<satellite_calculations::Satellite>::new());

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
                            // Calculate satellite positions and render frame
                            satellites_value.with_value(|satellites| {
                                if !satellites.is_empty() {
                                    let positions = satellite_calculations::calculate_positions(satellites);
                                    let sat_positions: Vec<satellite_calculations::SatellitePosition> =
                                        positions.into_iter().map(|(_, pos)| pos).collect();

                                    renderer_value.update_value(|renderer| {
                                        renderer.update_satellites(sat_positions);
                                        renderer.render();
                                    });
                                } else {
                                    renderer_value.update_value(|renderer| {
                                        renderer.render();
                                    });
                                }
                            });

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

    view! {
        <div class="w-full bg-gradient-to-br from-gray-900 to-black py-6 px-4 rounded-xl mb-8">
            <div class="flex items-center justify-between mb-4">
                <h2 class="text-xl font-bold text-white">
                    "Satellite Orbit Tracker"
                </h2>
                <div class="flex gap-2">
                    <select
                        class="px-3 py-1 rounded bg-gray-800 text-white text-sm border border-gray-700"
                        on:change=move |ev| {
                            let value = event_target_value(&ev);
                            set_selected_group.set(value);
                        }
                    >
                        <option value="starlink">"Starlink"</option>
                        <option value="active">"All Active"</option>
                        <option value="stations">"Space Stations"</option>
                        <option value="visual">"Visible"</option>
                        <option value="gps-ops">"GPS"</option>
                    </select>
                </div>
            </div>

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

                <div class="absolute bottom-4 left-4 bg-black/70 text-white px-3 py-2 rounded text-sm">
                    {move || {
                        let count = tle_data.get().len();
                        format!("Tracking {} satellites", count)
                    }}
                </div>
            </div>

            <div class="mt-4 text-xs text-gray-400">
                <p>"Data from CelesTrak • Updates every 5 minutes"</p>
            </div>
        </div>
    }
}
