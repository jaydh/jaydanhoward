//! Real visitor logging + world-map rendering, full production schema/query
//! fidelity — ported from the real site's `src/middleware/visitor_logger.rs`
//! + `src/db.rs` (insert_visit/get_visitor_stats) + `src/routes/world_map.rs`.
//! Local dev runs against a throwaway Postgres seeded with the exact
//! production migrations (`migrations/0001_create_visitors.sql`); staging
//! validation (milestone 8) points this at the real production DB.

use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::Response;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::net::IpAddr;

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPool::connect(database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

fn should_log(path: &str) -> bool {
    if path == "/health_check" || path.starts_with("/pkg/") {
        return false;
    }
    if path.contains('.') {
        if let Some(ext) = path.rsplit('.').next() {
            return !matches!(
                ext,
                "wasm" | "js" | "css" | "woff2" | "woff" | "ttf" | "eot" | "otf" | "png" | "jpg"
                    | "jpeg" | "gif" | "svg" | "ico" | "webp" | "map"
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

async fn record_visit(pool: PgPool, ip: String, path: String) {
    let is_private = is_private_ip(&ip);

    let geo = if !is_private {
        async {
            reqwest::Client::new()
                .get(format!(
                    "http://ip-api.com/json/{ip}?fields=status,country,countryCode,regionName,city,lat,lon,isp"
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
        .filter(|g| g.status == "success")
    } else {
        None
    };

    let (country, country_code, region, city, lat, lon, isp) = match geo {
        Some(g) => (g.country, g.country_code, g.region_name, g.city, g.lat, g.lon, g.isp),
        None => (None, None, None, None, None, None, None),
    };

    let _ = sqlx::query(
        r#"INSERT INTO visitors (ip, path, country, country_code, region, city, lat, lon, isp)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
    )
    .bind(&ip)
    .bind(&path)
    .bind(&country)
    .bind(&country_code)
    .bind(&region)
    .bind(&city)
    .bind(lat)
    .bind(lon)
    .bind(&isp)
    .execute(&pool)
    .await;
}

pub async fn visitor_logger(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();

    if should_log(&path) {
        let ip = headers
            .get("x-real-ip")
            .or_else(|| headers.get("x-forwarded-for"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
            .unwrap_or_else(|| "127.0.0.1".to_string());

        let pool = pool.clone();
        tokio::spawn(async move {
            record_visit(pool, ip, path).await;
        });
    }

    next.run(req).await
}

/// Real query logic ported verbatim from `src/db.rs::get_visitor_stats`.
pub fn fetch_visitor_stats(pool: &PgPool) -> Value {
    let result: Result<Value, sqlx::Error> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let unique_ips: i64 = sqlx::query_scalar(
                "SELECT COUNT(DISTINCT ip) FROM visitors WHERE visited_at > NOW() - INTERVAL '30 days'",
            )
            .fetch_one(pool)
            .await
            .unwrap_or(0);

            let unique_countries: i64 = sqlx::query_scalar(
                "SELECT COUNT(DISTINCT country_code) FROM visitors WHERE visited_at > NOW() - INTERVAL '30 days' AND country_code IS NOT NULL",
            )
            .fetch_one(pool)
            .await
            .unwrap_or(0);

            let total_visits: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM (SELECT DISTINCT ip, visited_at::date FROM visitors WHERE visited_at > NOW() - INTERVAL '30 days') sub",
            )
            .fetch_one(pool)
            .await
            .unwrap_or(0);

            let country_rows = sqlx::query(
                r#"SELECT COALESCE(country, 'Unknown') as country, COALESCE(country_code, 'XX') as country_code,
                          COUNT(DISTINCT ip) as count
                   FROM visitors
                   WHERE visited_at > NOW() - INTERVAL '30 days' AND country_code IS NOT NULL
                   GROUP BY country, country_code ORDER BY count DESC LIMIT 10"#,
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            let top_countries: Vec<Value> = country_rows
                .iter()
                .map(|row| {
                    json!({
                        "country": row.try_get::<String, _>("country").unwrap_or_default(),
                        "country_code": row.try_get::<String, _>("country_code").unwrap_or_default(),
                        "count": row.try_get::<i64, _>("count").unwrap_or(0),
                    })
                })
                .collect();

            let recent_rows = sqlx::query(
                r#"SELECT country, country_code, city, path, visited_at FROM (
                       SELECT DISTINCT ON (ip) country, country_code, city, path, visited_at
                       FROM visitors WHERE visited_at > NOW() - INTERVAL '30 days'
                       ORDER BY ip, visited_at DESC
                   ) deduped ORDER BY visited_at DESC LIMIT 20"#,
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            let now = Utc::now();
            let recent_visits: Vec<Value> = recent_rows
                .iter()
                .map(|row| {
                    let visited_at: DateTime<Utc> = row.try_get("visited_at").unwrap_or(now);
                    let minutes_ago = (now - visited_at).num_minutes().max(0);
                    let city: Option<String> = row.try_get("city").ok();
                    let country: Option<String> = row.try_get("country").ok();
                    let location = match (city, country) {
                        (Some(c), Some(co)) => format!("{c}, {co}"),
                        (None, Some(co)) => co,
                        (Some(c), None) => c,
                        (None, None) => "unknown".to_string(),
                    };
                    json!({
                        "path": row.try_get::<String, _>("path").unwrap_or_default(),
                        "location": location,
                        "minutes_ago": minutes_ago,
                    })
                })
                .collect();

            let point_rows = sqlx::query(
                r#"SELECT ROUND(lat::numeric, 1)::float8 as lat, ROUND(lon::numeric, 1)::float8 as lon
                   FROM visitors
                   WHERE visited_at > NOW() - INTERVAL '30 days' AND lat IS NOT NULL AND lon IS NOT NULL
                   GROUP BY ROUND(lat::numeric, 1), ROUND(lon::numeric, 1) LIMIT 500"#,
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            let points: Vec<Value> = point_rows
                .iter()
                .map(|row| {
                    json!({
                        "lat": row.try_get::<f64, _>("lat").unwrap_or(0.0),
                        "lon": row.try_get::<f64, _>("lon").unwrap_or(0.0),
                    })
                })
                .collect();

            Ok(json!({
                "connected": true,
                "unique_ips": unique_ips,
                "unique_countries": unique_countries,
                "total_visits": total_visits,
                "top_countries": top_countries,
                "recent_visits": recent_visits,
                "points": points,
            }))
        })
    });

    result.unwrap_or_else(|e| {
        json!({ "connected": false, "error": e.to_string(), "unique_ips": 0, "unique_countries": 0,
                "total_visits": 0, "top_countries": [], "recent_visits": [], "points": [] })
    })
}

// ── World map SVG ────────────────────────────────────────────────────────
// Ported verbatim from src/routes/world_map.rs: fetches Natural Earth land
// geometry once at startup and projects it to a simple equirectangular SVG,
// cached in memory and served at GET /world-map.svg.

const GEOJSON_URL: &str =
    "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_110m_land.geojson";

fn project(lon: f64, lat: f64) -> (f64, f64) {
    (lon + 180.0, 90.0 - lat)
}

fn ring_to_path(coords: &[Vec<f64>]) -> String {
    let mut s = String::new();
    for (i, c) in coords.iter().enumerate() {
        let (x, y) = project(c[0], c[1]);
        s.push_str(if i == 0 { "M" } else { "L" });
        s.push_str(&format!("{x:.2},{y:.2} "));
    }
    s.push('Z');
    s
}

fn geometry_paths(geometry: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    match geometry["type"].as_str() {
        Some("Polygon") => {
            if let Some(rings) = geometry["coordinates"].as_array() {
                for ring in rings {
                    if let Some(coords) = ring.as_array() {
                        let pts: Vec<Vec<f64>> = coords
                            .iter()
                            .filter_map(|p| p.as_array().map(|a| a.iter().filter_map(|v| v.as_f64()).collect()))
                            .collect();
                        paths.push(ring_to_path(&pts));
                    }
                }
            }
        }
        Some("MultiPolygon") => {
            if let Some(polys) = geometry["coordinates"].as_array() {
                for poly in polys {
                    if let Some(rings) = poly.as_array() {
                        for ring in rings {
                            if let Some(coords) = ring.as_array() {
                                let pts: Vec<Vec<f64>> = coords
                                    .iter()
                                    .filter_map(|p| p.as_array().map(|a| a.iter().filter_map(|v| v.as_f64()).collect()))
                                    .collect();
                                paths.push(ring_to_path(&pts));
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    paths
}

fn fallback_svg() -> String {
    r#"<svg viewBox="0 0 360 180" xmlns="http://www.w3.org/2000/svg"></svg>"#.to_string()
}

pub async fn fetch_world_map_svg(client: &reqwest::Client) -> String {
    let geojson = match client.get(GEOJSON_URL).send().await {
        Ok(r) => match r.json::<Value>().await {
            Ok(j) => j,
            Err(_) => return fallback_svg(),
        },
        Err(_) => return fallback_svg(),
    };

    let mut all_paths = String::new();
    if let Some(features) = geojson["features"].as_array() {
        for feature in features {
            for path in geometry_paths(&feature["geometry"]) {
                all_paths.push_str(&path);
                all_paths.push(' ');
            }
        }
    }

    format!(
        r#"<svg viewBox="0 0 360 180" xmlns="http://www.w3.org/2000/svg"><path d="{all_paths}" fill="currentColor" /></svg>"#
    )
}
