use axum::{extract::Request, middleware::Next, response::Response};
use std::net::IpAddr;

#[cfg(feature = "ssr")]
use sqlx::PgPool;

pub async fn visitor_logger_fn(
    pool: Option<PgPool>,
    http_client: reqwest::Client,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();

    if pool.is_none() || !should_log(&path) {
        return next.run(req).await;
    }

    let ip = {
        let headers = req.headers();
        headers
            .get("x-real-ip")
            .or_else(|| headers.get("x-forwarded-for"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    };

    if !is_private_ip(&ip) {
        let pool = pool.unwrap();
        tokio::task::spawn(async move {
            record_visit(pool, http_client, ip, path).await;
        });
    }

    next.run(req).await
}

fn should_log(path: &str) -> bool {
    if path == "/health_check" || path.starts_with("/jaydanhoward_wasm/") {
        return false;
    }
    if path.contains('.') {
        if let Some(ext) = path.rsplit('.').next() {
            return !matches!(
                ext,
                "wasm"
                    | "js"
                    | "css"
                    | "woff2"
                    | "woff"
                    | "ttf"
                    | "eot"
                    | "otf"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "svg"
                    | "ico"
                    | "webp"
                    | "map"
            );
        }
    }
    true
}

fn is_private_ip(ip_str: &str) -> bool {
    match ip_str.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Ok(IpAddr::V6(ip)) => ip.is_loopback(),
        Err(_) => true,
    }
}

#[cfg(feature = "ssr")]
async fn record_visit(pool: PgPool, http_client: reqwest::Client, ip: String, path: String) {
    use crate::db::insert_visit;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GeoResponse {
        status: String,
        country: Option<String>,
        country_code: Option<String>,
        region_name: Option<String>,
        city: Option<String>,
        lat: Option<f64>,
        lon: Option<f64>,
        isp: Option<String>,
    }

    let geo: Option<GeoResponse> = async {
        let resp = http_client
            .get(format!("http://ip-api.com/json/{ip}?fields=status,country,countryCode,regionName,city,lat,lon,isp"))
            .send()
            .await
            .ok()?;
        resp.json::<GeoResponse>().await.ok()
    }
    .await
    .filter(|g| g.status == "success");

    let (country, country_code, region, city, lat, lon, isp) = match geo {
        Some(g) => (
            g.country,
            g.country_code,
            g.region_name,
            g.city,
            g.lat,
            g.lon,
            g.isp,
        ),
        None => (None, None, None, None, None, None, None),
    };

    let _ = insert_visit(
        &pool,
        &ip,
        &path,
        country.as_deref(),
        country_code.as_deref(),
        region.as_deref(),
        city.as_deref(),
        lat,
        lon,
        isp.as_deref(),
    )
    .await;
}
