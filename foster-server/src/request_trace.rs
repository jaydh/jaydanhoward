//! Real per-request trace data — no Foster machine involved, matching the
//! plan's call that this section has nothing interactive/discrete-state
//! about it. A plain axum handler reads real request headers (this demo
//! isn't behind Cloudflare, so cf-* headers will simply be absent — that's
//! the honest local-dev answer, not faked) plus a real geo lookup against
//! ip-api.com for non-private IPs, same as the real site.

use axum::extract::ConnectInfo;
use axum::http::HeaderMap;
use axum::response::Json;
use serde::Serialize;
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

pub async fn get_request_trace(
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Json<RequestTraceData> {
    let ip = headers
        .get("cf-connecting-ip")
        .or_else(|| headers.get("x-real-ip"))
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| addr.ip().to_string());

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

    Json(RequestTraceData {
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
