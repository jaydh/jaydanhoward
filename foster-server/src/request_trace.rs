//! Real per-request trace data for the "How You Got Here" section: request
//! headers (cf-* ones are simply absent when not behind Cloudflare — the
//! honest local-dev answer, not faked) plus a real geo lookup against
//! ip-api.com for non-private IPs. Fed into the per-visitor "request_trace"
//! Foster machine via `Foster::request_event` (see main.rs), which runs
//! `trace` on every page load and every Refresh click.

use axum::http::HeaderMap;
use serde::Serialize;
use serde_json::{json, Value};
use std::net::SocketAddr;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeoResponse {
    status: String,
    country: Option<String>,
    city: Option<String>,
    isp: Option<String>,
}

pub async fn trace(headers: &HeaderMap, addr: Option<SocketAddr>) -> RequestTraceData {
    let ip = headers
        .get("cf-connecting-ip")
        .or_else(|| headers.get("x-real-ip"))
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .or_else(|| addr.map(|a| a.ip().to_string()))
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
        || ip.starts_with("::1")
        || ip.starts_with("10.")
        || ip.starts_with("192.168.")
        || ip.starts_with("172.");

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

    RequestTraceData {
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
    }
}

fn iata_city(code: &str) -> Option<&'static str> {
    Some(match code {
        "SJC" => "San Jose, CA", "LAX" => "Los Angeles, CA", "SFO" => "San Francisco, CA",
        "SEA" => "Seattle, WA", "DEN" => "Denver, CO", "DFW" => "Dallas, TX", "ORD" => "Chicago, IL",
        "ATL" => "Atlanta, GA", "IAD" => "Ashburn, VA", "EWR" => "Newark, NJ", "MIA" => "Miami, FL",
        "LHR" => "London, UK", "AMS" => "Amsterdam, NL", "FRA" => "Frankfurt, DE", "CDG" => "Paris, FR",
        "MAD" => "Madrid, ES", "MXP" => "Milan, IT", "ARN" => "Stockholm, SE", "SIN" => "Singapore",
        "NRT" => "Tokyo, JP", "HKG" => "Hong Kong", "SYD" => "Sydney, AU", "GRU" => "São Paulo, BR",
        "YYZ" => "Toronto, CA",
        _ => return None,
    })
}

/// "Chrome · macOS"-style summary of a User-Agent header.
fn summarize_ua(ua: &str) -> String {
    let browser = if ua.contains("Edg/") { "Edge" }
        else if ua.contains("Chrome/") { "Chrome" }
        else if ua.contains("Firefox/") { "Firefox" }
        else if ua.contains("Safari/") { "Safari" }
        else if ua.contains("curl/") { "curl" }
        else { "Unknown" };
    let os = if ua.contains("Windows") { "Windows" }
        else if ua.contains("iPhone") || ua.contains("iPad") { "iOS" }
        else if ua.contains("Android") { "Android" }
        else if ua.contains("Mac OS X") { "macOS" }
        else if ua.contains("Linux") { "Linux" }
        else { "Unknown" };
    format!("{browser} · {os}")
}

/// Display fields for the trace cards. Missing values are `""`, which the
/// markup's `fx-if` hides (rows with no value are omitted, as before).
pub fn trace_view(t: &RequestTraceData) -> Value {
    let location = match (&t.geo_city, &t.geo_country) {
        (Some(city), Some(country)) => format!("{city}, {country}"),
        (None, Some(x)) | (Some(x), None) => x.clone(),
        (None, None) => String::new(),
    };
    let pop = t.cf_datacenter.as_deref().map(|dc| match iata_city(dc) {
        Some(city) => format!("{dc} · {city}"),
        None => dc.to_string(),
    });
    json!({
        "ip": t.ip,
        "location": location,
        "isp": t.geo_isp.clone().unwrap_or_default(),
        "client": t.user_agent.as_deref().map(summarize_ua).unwrap_or_default(),
        "protocol": if t.https { "HTTPS · TLS 1.3" } else { "HTTP" },
        "pop": pop.unwrap_or_default(),
        "ray": t.cf_ray.clone().unwrap_or_default(),
        "namespace": t.namespace.clone().unwrap_or_default(),
        "pod": t.pod_name,
        "node": t.node_name.clone().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ua_summary() {
        let mac_chrome = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0 Safari/537.36";
        assert_eq!(summarize_ua(mac_chrome), "Chrome · macOS");
        assert_eq!(summarize_ua("curl/8.0"), "curl · Unknown");
    }

    #[tokio::test]
    async fn trace_from_headers() {
        let mut h = HeaderMap::new();
        h.insert("x-real-ip", "127.0.0.1".parse().unwrap());
        h.insert("cf-ray", "abc123-SJC".parse().unwrap());
        h.insert("cf-visitor", r#"{"scheme":"https"}"#.parse().unwrap());
        let v = trace_view(&trace(&h, None).await);
        assert_eq!(v["ip"], "127.0.0.1");
        assert_eq!(v["pop"], "SJC · San Jose, CA");
        assert_eq!(v["protocol"], "HTTPS · TLS 1.3");
        assert_eq!(v["location"], "");
    }
}
