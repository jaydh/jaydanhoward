use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IpVisit {
    pub path: String,
    pub minutes_ago: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IpInfo {
    pub ip: String,
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub isp: Option<String>,
    pub history: Vec<IpVisit>,
}

#[server(name = GetMyInfo, prefix = "/api", endpoint = "get_my_info")]
pub async fn get_my_info() -> Result<IpInfo, ServerFnError<String>> {
    use actix_web::{web::Data, HttpRequest};
    use leptos_actix::extract;
    use sqlx::PgPool;

    let req = extract::<HttpRequest>()
        .await
        .map_err(|e| ServerFnError::ServerError(format!("{e}")))?;

    let ip = {
        let raw = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();
        raw.parse::<std::net::SocketAddr>()
            .map(|s| s.ip().to_string())
            .unwrap_or(raw)
    };

    let pool = match extract::<Data<PgPool>>().await {
        Ok(p) => p,
        Err(_) => {
            return Ok(IpInfo {
                ip,
                country: None,
                country_code: None,
                city: None,
                region: None,
                isp: None,
                history: vec![],
            })
        }
    };

    crate::db::get_ip_info(&pool, &ip)
        .await
        .map(|info| IpInfo {
            ip: info.ip,
            country: info.country,
            country_code: info.country_code,
            city: info.city,
            region: info.region,
            isp: info.isp,
            history: info
                .history
                .into_iter()
                .map(|v| IpVisit { path: v.path, minutes_ago: v.minutes_ago })
                .collect(),
        })
        .map_err(|e| ServerFnError::ServerError(format!("DB error: {e}")))
}

#[server(name = ForgetMe, prefix = "/api", endpoint = "forget_me")]
pub async fn forget_me() -> Result<(), ServerFnError<String>> {
    use actix_web::HttpRequest;
    use leptos_actix::extract;
    use actix_web::web::Data;
    use sqlx::PgPool;

    let req = extract::<HttpRequest>()
        .await
        .map_err(|e| ServerFnError::ServerError(format!("{e}")))?;

    let ip = {
        let raw = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();
        raw.parse::<std::net::SocketAddr>()
            .map(|s| s.ip().to_string())
            .unwrap_or(raw)
    };

    let pool = extract::<Data<PgPool>>()
        .await
        .map_err(|e| ServerFnError::ServerError(format!("{e}")))?;

    crate::db::delete_ip_visits(&pool, &ip)
        .await
        .map_err(|e| ServerFnError::ServerError(format!("DB error: {e}")))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CountryStat {
    pub country: String,
    pub country_code: String,
    pub count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecentVisit {
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub path: String,
    pub minutes_ago: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisitorPoint {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisitorStats {
    pub unique_ips: i64,
    pub unique_countries: i64,
    pub total_visits: i64,
    pub top_countries: Vec<CountryStat>,
    pub recent_visits: Vec<RecentVisit>,
    pub points: Vec<VisitorPoint>,
}

#[server(name = GetVisitorStats, prefix = "/api", endpoint = "get_visitor_stats")]
pub async fn get_visitor_stats() -> Result<VisitorStats, ServerFnError<String>> {
    use crate::db::get_visitor_stats as db_get_stats;
    use actix_web::web::Data;
    use leptos_actix::extract;
    use sqlx::PgPool;

    let pool = match extract::<Data<PgPool>>().await {
        Ok(p) => p,
        Err(_) => {
            return Ok(VisitorStats {
                unique_ips: 0,
                unique_countries: 0,
                total_visits: 0,
                top_countries: vec![],
                recent_visits: vec![],
                points: vec![],
            })
        }
    };

    db_get_stats(&pool)
        .await
        .map(|s| VisitorStats {
            unique_ips: s.unique_ips,
            unique_countries: s.unique_countries,
            total_visits: s.total_visits,
            top_countries: s
                .top_countries
                .into_iter()
                .map(|c| CountryStat {
                    country: c.country,
                    country_code: c.country_code,
                    count: c.count,
                })
                .collect(),
            recent_visits: s
                .recent_visits
                .into_iter()
                .map(|v| RecentVisit {
                    country: v.country,
                    country_code: v.country_code,
                    city: v.city,
                    path: v.path,
                    minutes_ago: v.minutes_ago,
                })
                .collect(),
            points: s
                .points
                .into_iter()
                .map(|p| VisitorPoint {
                    lat: p.lat,
                    lon: p.lon,
                })
                .collect(),
        })
        .map_err(|e| ServerFnError::ServerError(format!("DB error: {e}")))
}

fn country_flag(code: &str) -> String {
    code.to_uppercase()
        .chars()
        .filter_map(|c| {
            let offset = c as u32;
            if offset >= 'A' as u32 && offset <= 'Z' as u32 {
                char::from_u32(0x1F1E6 + (offset - 'A' as u32))
            } else {
                None
            }
        })
        .collect()
}

fn format_time_ago(minutes: i64) -> String {
    if minutes < 1 {
        "just now".to_string()
    } else if minutes < 60 {
        format!("{minutes}m ago")
    } else if minutes < 1440 {
        format!("{}h ago", minutes / 60)
    } else {
        format!("{}d ago", minutes / 1440)
    }
}

#[component]
fn YourVisit() -> impl IntoView {
    let info = Resource::new(|| (), |_| get_my_info());
    let forget = Action::new(|_: &()| forget_me());

    view! {
        <Suspense fallback=|| ()>
            {move || {
                // Hide the card once forget_me succeeds
                if forget.value().get().is_some_and(|r| r.is_ok()) {
                    return Some(view! { <div></div> }.into_any());
                }
                info.get().map(|result| match result {
                    Err(_) => view! { <div></div> }.into_any(),
                    Ok(info) => {
                        let location = match (&info.city, &info.region, &info.country) {
                            (Some(city), Some(region), _) => format!("{city}, {region}"),
                            (Some(city), _, Some(country)) => format!("{city}, {country}"),
                            (_, _, Some(country)) => country.clone(),
                            _ => String::new(),
                        };
                        let flag = info.country_code.as_deref()
                            .map(country_flag)
                            .unwrap_or_default();
                        let history = info.history.clone();
                        view! {
                            <div class="bg-surface border border-border rounded-lg p-3 font-mono text-xs space-y-2">
                                // Header row: IP · location · ISP · forget button
                                <div class="flex items-center gap-2 text-charcoal-lighter flex-wrap">
                                    <span class="text-charcoal">"You: "</span>
                                    <span class="text-accent">{info.ip}</span>
                                    {if !location.is_empty() {
                                        view! {
                                            <span class="text-charcoal-lighter">"·"</span>
                                            <span>{flag}" "{location}</span>
                                        }.into_any()
                                    } else {
                                        view! { <span></span> }.into_any()
                                    }}
                                    {info.isp.map(|isp| view! {
                                        <span class="text-charcoal-lighter">"·"</span>
                                        <span class="truncate opacity-70">{isp}</span>
                                    })}
                                    <button
                                        class="ml-auto text-charcoal-lighter hover:text-red-400 transition-colors"
                                        title="Delete all records of your visits"
                                        on:click=move |_| { forget.dispatch(()); }
                                    >
                                        "forget me"
                                    </button>
                                </div>
                            // Visit history
                            {(!history.is_empty()).then(|| view! {
                                <div class="space-y-1 pt-1 border-t border-border">
                                    {history.into_iter().map(|v| {
                                        let time = format_time_ago(v.minutes_ago);
                                        view! {
                                            <div class="flex items-center gap-2 text-charcoal-lighter">
                                                <span class="text-charcoal truncate flex-1">{v.path}</span>
                                                <span class="flex-shrink-0 opacity-60">{time}</span>
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            })}
                        </div>
                    }.into_any()
                }
            })}}
        </Suspense>
    }
}

#[component]
fn WorldMap(points: Vec<VisitorPoint>) -> impl IntoView {
    // Equirectangular projection: lon [-180,180] → x [0,360], lat [90,-90] → y [0,180]
    // x = lon + 180, y = 90 - lat
    let dot_elements: Vec<_> = points
        .iter()
        .map(|p| {
            let x = p.lon + 180.0;
            let y = 90.0 - p.lat;
            view! {
                <circle
                    cx={format!("{x:.2}")}
                    cy={format!("{y:.2}")}
                    r="1.8"
                    fill="#3b82f6"
                    fill-opacity="0.9"
                />
            }
        })
        .collect();

    let is_empty = points.is_empty();

    view! {
        <div class="relative w-full rounded-lg overflow-hidden border border-border" style="aspect-ratio: 2/1;">
            <img
                src="/world-map.svg"
                class="absolute inset-0 w-full h-full"
                style="object-fit: fill;"
                alt=""
            />
            <svg
                viewBox="0 0 360 180"
                class="absolute inset-0 w-full h-full"
                preserveAspectRatio="xMidYMid meet"
                xmlns="http://www.w3.org/2000/svg"
            >
                {dot_elements}
            </svg>
            {is_empty.then(|| view! {
                <div class="absolute inset-0 flex items-center justify-center">
                    <p class="text-xs text-charcoal-lighter">"No location data yet"</p>
                </div>
            })}
        </div>
    }
}

#[component]
pub fn Visitors() -> impl IntoView {
    let stats = Resource::new(|| (), |_| get_visitor_stats());

    view! {
        <div class="w-full">
            <div class="flex items-center justify-between mb-4">
                <h2 class="text-xl font-bold text-charcoal">"Visitors"</h2>
                <span class="text-xs text-charcoal-lighter">"last 30 days"</span>
            </div>
            <div class="mb-6">
                <YourVisit/>
            </div>

            <Suspense fallback=|| view! {
                <div class="text-center text-charcoal-lighter py-12">"Loading visitor data..."</div>
            }>
                {move || {
                    stats.get().map(|result| match result {
                        Err(_) => view! {
                            <div class="text-center text-charcoal-lighter py-12">
                                "Visitor stats unavailable"
                            </div>
                        }.into_any(),
                        Ok(s) => {
                            let max_count = s.top_countries.iter().map(|c| c.count).max().unwrap_or(1);
                            let countries = s.top_countries.clone();
                            let recent = s.recent_visits.clone();
                            let points = s.points.clone();

                            view! {
                                <div class="space-y-6">
                                    // Summary stats
                                    <div class="grid grid-cols-3 gap-4">
                                        <div class="bg-surface border border-border rounded-lg p-4 text-center">
                                            <div class="text-3xl font-bold text-accent">{s.total_visits}</div>
                                            <div class="text-xs text-charcoal-lighter mt-1">"total visits"</div>
                                        </div>
                                        <div class="bg-surface border border-border rounded-lg p-4 text-center">
                                            <div class="text-3xl font-bold text-blue-500">{s.unique_ips}</div>
                                            <div class="text-xs text-charcoal-lighter mt-1">"unique visitors"</div>
                                        </div>
                                        <div class="bg-surface border border-border rounded-lg p-4 text-center">
                                            <div class="text-3xl font-bold text-green-600">{s.unique_countries}</div>
                                            <div class="text-xs text-charcoal-lighter mt-1">"countries"</div>
                                        </div>
                                    </div>

                                    // World map
                                    <WorldMap points=points />

                                    // Countries + recent side by side
                                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                                        // Top countries
                                        <div class="bg-surface border border-border rounded-lg p-4">
                                            <h3 class="text-sm font-medium text-charcoal-lighter mb-3">"Top Countries"</h3>
                                            <div class="space-y-2">
                                                {countries.into_iter().map(|c| {
                                                    let pct = (c.count as f64 / max_count as f64 * 100.0) as u32;
                                                    let flag = country_flag(&c.country_code);
                                                    view! {
                                                        <div class="flex items-center gap-2 text-sm">
                                                            <span class="text-base w-6 flex-shrink-0">{flag}</span>
                                                            <span class="text-charcoal flex-1 truncate">{c.country}</span>
                                                            <div class="w-20 bg-border rounded-full h-1.5 flex-shrink-0">
                                                                <div
                                                                    class="bg-blue-500 h-1.5 rounded-full"
                                                                    style={format!("width: {pct}%")}
                                                                ></div>
                                                            </div>
                                                            <span class="text-charcoal-lighter text-xs w-8 text-right flex-shrink-0">
                                                                {c.count}
                                                            </span>
                                                        </div>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        </div>

                                        // Recent visitors
                                        <div class="bg-surface border border-border rounded-lg p-4">
                                            <h3 class="text-sm font-medium text-charcoal-lighter mb-3">"Recent Visitors"</h3>
                                            <div class="space-y-2">
                                                {recent.into_iter().map(|v| {
                                                    let flag = v.country_code.as_deref()
                                                        .map(country_flag)
                                                        .unwrap_or_else(|| "🌐".to_string());
                                                    let location = match (&v.city, &v.country) {
                                                        (Some(city), Some(country)) => format!("{city}, {country}"),
                                                        (None, Some(country)) => country.clone(),
                                                        _ => "Unknown".to_string(),
                                                    };
                                                    let time = format_time_ago(v.minutes_ago);
                                                    view! {
                                                        <div class="flex items-center gap-2 text-sm">
                                                            <span class="text-base w-6 flex-shrink-0">{flag}</span>
                                                            <span class="text-charcoal flex-1 truncate">{location}</span>
                                                            <span class="text-charcoal-lighter text-xs flex-shrink-0">{time}</span>
                                                        </div>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            }.into_any()
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}
