use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestTraceData {
    pub ip: String,
    pub geo_country: Option<String>,
    pub geo_city: Option<String>,
    pub geo_isp: Option<String>,
    pub user_agent: Option<String>,
    pub cf_ray: Option<String>,
    pub cf_datacenter: Option<String>,
    pub https: bool,
    pub pod_name: String,
    pub node_name: Option<String>,
    pub namespace: Option<String>,
}

#[server(name = GetRequestTrace, prefix = "/api", endpoint = "request_trace")]
pub async fn get_request_trace() -> Result<RequestTraceData, ServerFnError<String>> {
    use axum::http::HeaderMap;
    use leptos_axum::extract;
    use serde::Deserialize as SerdeDeserialize;

    let headers = extract::<HeaderMap>().await.unwrap_or_default();

    let ip = headers
        .get("cf-connecting-ip")
        .or_else(|| headers.get("x-real-ip"))
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let cf_ray = headers
        .get("cf-ray")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let cf_datacenter = cf_ray
        .as_deref()
        .and_then(|ray| ray.rsplit('-').next())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let https = headers
        .get("cf-visitor")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("\"https\""))
        .unwrap_or(false);

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let is_private = ip == "unknown"
        || ip.starts_with("127.")
        || ip.starts_with("10.")
        || ip.starts_with("192.168.")
        || ip.starts_with("172.");

    #[derive(SerdeDeserialize)]
    #[serde(rename_all = "camelCase")]
    struct GeoResponse {
        status: String,
        country: Option<String>,
        city: Option<String>,
        isp: Option<String>,
    }

    let (geo_country, geo_city, geo_isp) = if !is_private {
        let geo: Option<GeoResponse> = async {
            reqwest::Client::new()
                .get(format!(
                    "http://ip-api.com/json/{ip}?fields=status,country,city,isp"
                ))
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await
                .ok()?
                .json::<GeoResponse>()
                .await
                .ok()
        }
        .await
        .filter(|g| g.status == "success");

        match geo {
            Some(g) => (g.country, g.city, g.isp),
            None => (None, None, None),
        }
    } else {
        (None, None, None)
    };

    let pod_name = std::env::var("HOSTNAME").unwrap_or_else(|_| "local-dev".to_string());
    let node_name = std::env::var("MY_NODE_NAME").ok();
    let namespace = std::env::var("MY_NAMESPACE").ok();

    Ok(RequestTraceData {
        ip,
        geo_country,
        geo_city,
        geo_isp,
        user_agent,
        cf_ray,
        cf_datacenter,
        https,
        pod_name,
        node_name,
        namespace,
    })
}

fn iata_to_city(iata: &str) -> Option<&'static str> {
    match iata {
        "SJC" => Some("San Jose, CA"),
        "LAX" => Some("Los Angeles, CA"),
        "SFO" => Some("San Francisco, CA"),
        "SEA" => Some("Seattle, WA"),
        "DEN" => Some("Denver, CO"),
        "DFW" => Some("Dallas, TX"),
        "ORD" => Some("Chicago, IL"),
        "ATL" => Some("Atlanta, GA"),
        "IAD" => Some("Ashburn, VA"),
        "EWR" => Some("Newark, NJ"),
        "MIA" => Some("Miami, FL"),
        "LHR" => Some("London, UK"),
        "AMS" => Some("Amsterdam, NL"),
        "FRA" => Some("Frankfurt, DE"),
        "CDG" => Some("Paris, FR"),
        "MAD" => Some("Madrid, ES"),
        "MXP" => Some("Milan, IT"),
        "ARN" => Some("Stockholm, SE"),
        "SIN" => Some("Singapore"),
        "NRT" => Some("Tokyo, JP"),
        "HKG" => Some("Hong Kong"),
        "SYD" => Some("Sydney, AU"),
        "GRU" => Some("São Paulo, BR"),
        "YYZ" => Some("Toronto, CA"),
        _ => None,
    }
}

fn summarize_ua(ua: &str) -> String {
    let browser = if ua.contains("Edg/") {
        "Edge"
    } else if ua.contains("Chrome/") {
        "Chrome"
    } else if ua.contains("Firefox/") {
        "Firefox"
    } else if ua.contains("Safari/") && !ua.contains("Chrome") {
        "Safari"
    } else if ua.contains("curl/") {
        "curl"
    } else {
        "Unknown"
    };

    let os = if ua.contains("Windows") {
        "Windows"
    } else if ua.contains("iPhone") || ua.contains("iPad") {
        "iOS"
    } else if ua.contains("Android") {
        "Android"
    } else if ua.contains("Mac OS X") {
        "macOS"
    } else if ua.contains("Linux") {
        "Linux"
    } else {
        "Unknown"
    };

    format!("{browser} · {os}")
}

#[component]
pub fn RequestTrace() -> impl IntoView {
    let trace = Resource::new(|| (), |_| get_request_trace());

    view! {
        <div class="w-full max-w-xl mx-auto">
            <Suspense fallback=|| view! {
                <div class="text-charcoal-lighter text-sm text-center py-8 font-mono">
                    "tracing your request..."
                </div>
            }>
                {move || trace.get().map(|result| match result {
                    Err(_) => view! {
                        <div class="text-red-500 text-sm text-center">"Failed to trace request."</div>
                    }.into_any(),
                    Ok(data) => view! { <TraceView data=data /> }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn TraceView(data: RequestTraceData) -> impl IntoView {
    let cf_location = data.cf_datacenter.as_deref().map(|iata| {
        if let Some(city) = iata_to_city(iata) {
            format!("{iata} · {city}")
        } else {
            iata.to_string()
        }
    });

    let ua_summary = data.user_agent.as_deref().map(summarize_ua);

    let geo_location = match (&data.geo_city, &data.geo_country) {
        (Some(city), Some(country)) => Some(format!("{city}, {country}")),
        (None, Some(country)) => Some(country.clone()),
        (Some(city), None) => Some(city.clone()),
        _ => None,
    };

    let protocol = if data.https { "HTTPS · TLS 1.3" } else { "HTTP" };

    view! {
        <div class="flex flex-col items-center">

            <TraceCard
                label="You"
                rows=vec![
                    Some(("ip".to_string(),       data.ip.clone())),
                    geo_location.map(|g|          ("location".to_string(), g)),
                    data.geo_isp.clone().map(|s|  ("isp".to_string(), s)),
                    ua_summary.map(|u|            ("client".to_string(), u)),
                ].into_iter().flatten().collect()
            />

            <Hop label=protocol />

            <TraceCard
                label="Cloudflare"
                rows=vec![
                    cf_location.clone().map(|l|      ("pop".to_string(), l)),
                    data.cf_ray.clone().map(|r|      ("ray".to_string(), r)),
                    Some(("shield".to_string(),       "WAF · DDoS · Bot Management".to_string())),
                ].into_iter().flatten().collect()
            />

            <Hop label="Zero Trust Tunnel (cloudflared)" />

            <TraceCard
                label="Homelab Cluster"
                rows=vec![
                    Some(("tunnel".to_string(),  "cloudflared → k8s Service".to_string())),
                    data.namespace.clone().map(|ns| ("namespace".to_string(), ns)),
                    Some(("service".to_string(), "jaydanhoward".to_string())),
                    Some(("infra".to_string(),   "Rook-Ceph · Harbor · Flux".to_string())),
                ].into_iter().flatten().collect()
            />

            <Hop label="kube-proxy → Pod" />

            <TraceCard
                label="This Pod"
                rows=vec![
                    Some(("pod".to_string(),   data.pod_name.clone())),
                    data.node_name.clone().map(|n| ("node".to_string(), n)),
                    Some(("runtime".to_string(), "Rust · Leptos · Axum".to_string())),
                    Some(("wasm".to_string(),    "SSR + WASM hydration via Bazel".to_string())),
                ].into_iter().flatten().collect()
            />

        </div>
    }
}

#[component]
fn TraceCard(label: &'static str, rows: Vec<(String, String)>) -> impl IntoView {
    view! {
        <div class="w-full bg-surface border border-border rounded-lg p-4">
            <p class="text-xs font-semibold text-accent uppercase tracking-widest mb-3">{label}</p>
            <div class="space-y-2">
                {rows.into_iter().map(|(k, v)| view! {
                    <div class="flex gap-3 text-xs">
                        <span class="text-charcoal-lighter w-20 shrink-0 font-mono">{k}</span>
                        <span class="text-charcoal font-mono break-all">{v}</span>
                    </div>
                }).collect_view()}
            </div>
        </div>
    }
}

#[component]
fn Hop(label: &'static str) -> impl IntoView {
    view! {
        <div class="flex flex-col items-center py-0.5 select-none">
            <div class="w-px h-4 bg-border" />
            <span class="text-[10px] text-charcoal-lighter font-mono px-2 py-0.5 bg-background border border-border rounded">
                {label}
            </span>
            <div class="w-px h-4 bg-border" />
        </div>
    }
}
