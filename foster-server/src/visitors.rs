//! Visitor logging + world-map rendering — ported from the real site's
//! `src/middleware/visitor_logger.rs` + `src/db.rs` + `src/routes/world_map.rs`,
//! then reworked to tell people from bots: only page views and probes are
//! logged (not Foster's own `/state`/`/events` traffic or the APIs), each
//! row is classified `is_bot`, geo lookups are cached per IP in `ip_geo`,
//! and rows older than [`RETENTION_DAYS`] are deleted daily.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{FromRow, PgPool, Row};
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPool::connect(database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Days of `visitors` (and cached `ip_geo`) history kept.
const RETENTION_DAYS: i32 = 90;
/// ip-api.com lookups per minute per replica. The free tier allows 45/min
/// per source IP, and every replica shares one egress IP.
const GEO_LOOKUPS_PER_MIN: u32 = 12;
/// Visits buffered for the writer task; past this (a flood) they're dropped
/// rather than queued without bound.
const QUEUE_CAPACITY: usize = 1024;
/// Longest user-agent/referer/path stored, so junk headers can't bloat rows.
const MAX_FIELD_LEN: usize = 512;

/// The site's own machinery rather than page views: Foster's state/SSE
/// endpoints, the JSON/upload APIs, and static assets.
fn is_internal(path: &str) -> bool {
    if matches!(path, "/state" | "/events" | "/transition" | "/health_check" | "/world-map.svg")
        || ["/api/", "/pkg/", "/widgets/"].iter().any(|p| path.starts_with(p))
    {
        return true;
    }
    let last = path.rsplit('/').next().unwrap_or(path);
    last.rsplit_once('.').is_some_and(|(_, ext)| {
        matches!(
            ext,
            "wasm" | "js" | "css" | "woff2" | "woff" | "ttf" | "eot" | "otf" | "png" | "jpg"
                | "jpeg" | "gif" | "svg" | "ico" | "webp" | "map"
        )
    })
}

/// Page views, plus anything that 404s — a 404 on an internal-looking path
/// (`/api/.env`, `/wp-content/x.js`) is still a probe worth seeing.
fn should_log(path: &str, status: u16) -> bool {
    status == 404 || !is_internal(path)
}

const BOT_UA_MARKERS: &[&str] = &[
    "bot", "crawl", "spider", "slurp", "headless", "lighthouse", "scan", "curl", "wget",
    "python", "go-http", "okhttp", "axios", "node-fetch", "libwww", "httpclient", "java/",
    "zgrab", "masscan", "nuclei", "httpx", "censys", "facebookexternalhit",
];

/// Request-level bot signals: client errors (probes for files that don't
/// exist), a missing or non-browser user agent, or a known bot/tool marker.
/// The writer also counts datacenter IPs (ip-api's `hosting`) as bots.
fn looks_automated(user_agent: Option<&str>, status: u16) -> bool {
    if (400..500).contains(&status) {
        return true;
    }
    let Some(ua) = user_agent.filter(|ua| !ua.is_empty()) else {
        return true;
    };
    let ua = ua.to_ascii_lowercase();
    !ua.starts_with("mozilla/") || BOT_UA_MARKERS.iter().any(|m| ua.contains(m))
}

fn is_private_ip(ip_str: &str) -> bool {
    match ip_str.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Ok(IpAddr::V6(ip)) => ip.is_loopback(),
        Err(_) => true,
    }
}

fn truncate(s: &str) -> String {
    s.chars().take(MAX_FIELD_LEN).collect()
}

struct Visit {
    ip: String,
    path: String,
    user_agent: Option<String>,
    referer: Option<String>,
    status: u16,
}

/// Middleware state: the writer task's queue, plus IPs never logged.
#[derive(Clone)]
pub struct VisitorLog {
    tx: mpsc::Sender<Visit>,
    ignore_ips: Arc<HashSet<String>>,
}

impl VisitorLog {
    /// Starts the writer and daily retention tasks. `VISITOR_IGNORE_IPS`
    /// (comma-separated) lists IPs that are never logged, e.g. your own.
    pub fn spawn(pool: PgPool) -> Self {
        let (tx, rx) = mpsc::channel(QUEUE_CAPACITY);
        tokio::spawn(write_visits(pool.clone(), rx));
        tokio::spawn(prune_old_visits(pool));
        let ignore_ips = std::env::var("VISITOR_IGNORE_IPS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        Self { tx, ignore_ips: Arc::new(ignore_ips) }
    }
}

pub async fn visitor_logger(State(log): State<VisitorLog>, req: Request, next: Next) -> Response {
    let path = truncate(req.uri().path());
    let (ip, user_agent, referer) = {
        let header = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok()).map(truncate);
        let ip = header("cf-connecting-ip")
            .or_else(|| header("x-real-ip"))
            .or_else(|| header("x-forwarded-for"))
            .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
            .unwrap_or_else(|| "127.0.0.1".to_string());
        (ip, header("user-agent"), header("referer"))
    };

    let response = next.run(req).await;

    let status = response.status().as_u16();
    if should_log(&path, status) && !log.ignore_ips.contains(&ip) {
        let _ = log.tx.try_send(Visit { ip, path, user_agent, referer, status });
    }
    response
}

/// Per-replica allowance for ip-api lookups: a fixed number per minute, and
/// a pause whenever ip-api says the shared quota is spent.
struct GeoBudget {
    window_start: Instant,
    used: u32,
    paused_until: Option<Instant>,
}

impl GeoBudget {
    fn new() -> Self {
        Self { window_start: Instant::now(), used: 0, paused_until: None }
    }

    fn take(&mut self, now: Instant) -> bool {
        if self.paused_until.is_some_and(|t| now < t) {
            return false;
        }
        if now.duration_since(self.window_start) >= Duration::from_secs(60) {
            self.window_start = now;
            self.used = 0;
        }
        if self.used >= GEO_LOOKUPS_PER_MIN {
            return false;
        }
        self.used += 1;
        true
    }

    fn pause(&mut self, for_: Duration) {
        self.paused_until = Some(Instant::now() + for_);
    }
}

#[derive(Default, FromRow)]
struct Geo {
    country: Option<String>,
    country_code: Option<String>,
    region: Option<String>,
    city: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
    isp: Option<String>,
    hosting: Option<bool>,
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
    hosting: Option<bool>,
}

async fn lookup_geo(client: &reqwest::Client, budget: &mut GeoBudget, ip: &str) -> Option<Geo> {
    let resp = client
        .get(format!(
            "http://ip-api.com/json/{ip}?fields=status,country,countryCode,regionName,city,lat,lon,isp,hosting"
        ))
        .send()
        .await
        .ok()?;
    // X-Rl: requests left in this window; X-Ttl: seconds until it resets.
    let header_num = |name: &str| -> Option<u64> { resp.headers().get(name)?.to_str().ok()?.parse().ok() };
    if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS || header_num("x-rl") == Some(0) {
        budget.pause(Duration::from_secs(header_num("x-ttl").unwrap_or(60)));
    }
    let g = resp.json::<GeoResponse>().await.ok().filter(|g| g.status == "success")?;
    Some(Geo {
        country: g.country,
        country_code: g.country_code,
        region: g.region_name,
        city: g.city,
        lat: g.lat,
        lon: g.lon,
        isp: g.isp,
        hosting: g.hosting,
    })
}

/// Cached geo for `ip`, refreshed from ip-api when the cache entry is
/// missing, stale, or predates the `hosting` flag — budget permitting.
async fn geo_for(pool: &PgPool, client: &reqwest::Client, budget: &mut GeoBudget, ip: &str) -> Option<Geo> {
    let row = sqlx::query(
        r#"SELECT country, country_code, region, city, lat, lon, isp, hosting,
                  (hosting IS NOT NULL AND looked_up_at > NOW() - INTERVAL '30 days') AS fresh
           FROM ip_geo WHERE ip = $1"#,
    )
    .bind(ip)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    let fresh = row.as_ref().is_some_and(|r| r.try_get("fresh").unwrap_or(false));
    let cached = row.and_then(|r| Geo::from_row(&r).ok());
    if fresh || !budget.take(Instant::now()) {
        return cached;
    }
    let Some(geo) = lookup_geo(client, budget, ip).await else {
        return cached;
    };
    let _ = sqlx::query(
        r#"INSERT INTO ip_geo (ip, country, country_code, region, city, lat, lon, isp, hosting)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           ON CONFLICT (ip) DO UPDATE SET
               country = EXCLUDED.country, country_code = EXCLUDED.country_code,
               region = EXCLUDED.region, city = EXCLUDED.city, lat = EXCLUDED.lat,
               lon = EXCLUDED.lon, isp = EXCLUDED.isp, hosting = EXCLUDED.hosting,
               looked_up_at = NOW()"#,
    )
    .bind(ip)
    .bind(&geo.country)
    .bind(&geo.country_code)
    .bind(&geo.region)
    .bind(&geo.city)
    .bind(geo.lat)
    .bind(geo.lon)
    .bind(&geo.isp)
    .bind(geo.hosting)
    .execute(pool)
    .await;
    Some(geo)
}

/// Single writer, so a burst of requests is serialized through one geo
/// budget and one DB connection instead of a task per request.
async fn write_visits(pool: PgPool, mut rx: mpsc::Receiver<Visit>) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap_or_default();
    let mut budget = GeoBudget::new();
    while let Some(v) = rx.recv().await {
        let geo = if is_private_ip(&v.ip) {
            Geo::default()
        } else {
            geo_for(&pool, &client, &mut budget, &v.ip).await.unwrap_or_default()
        };
        // A session-level verdict from visitor_review beats the per-request
        // heuristic (unclear verdicts leave the heuristic in charge).
        let reviewed: Option<bool> =
            sqlx::query_scalar("SELECT verdict <> 'human' FROM ip_verdicts WHERE ip = $1 AND verdict <> 'unclear'")
                .bind(&v.ip)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten();
        let is_bot = reviewed
            .unwrap_or_else(|| looks_automated(v.user_agent.as_deref(), v.status) || geo.hosting == Some(true));
        let result = sqlx::query(
            r#"INSERT INTO visitors (ip, path, country, country_code, region, city, lat, lon, isp,
                                     user_agent, referer, status, is_bot)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"#,
        )
        .bind(&v.ip)
        .bind(&v.path)
        .bind(&geo.country)
        .bind(&geo.country_code)
        .bind(&geo.region)
        .bind(&geo.city)
        .bind(geo.lat)
        .bind(geo.lon)
        .bind(&geo.isp)
        .bind(&v.user_agent)
        .bind(&v.referer)
        .bind(v.status as i16)
        .bind(is_bot)
        .execute(&pool)
        .await;
        if let Err(e) = result {
            eprintln!("visitor insert failed: {e}");
        }
    }
}

/// Deletes visits and cached geo older than [`RETENTION_DAYS`], at startup
/// and then daily. Idempotent, so every replica running it is harmless.
async fn prune_old_visits(pool: PgPool) {
    let mut interval = tokio::time::interval(Duration::from_secs(24 * 60 * 60));
    loop {
        interval.tick().await;
        for sql in [
            "DELETE FROM visitors WHERE visited_at < NOW() - make_interval(days => $1)",
            "DELETE FROM ip_geo WHERE looked_up_at < NOW() - make_interval(days => $1)",
        ] {
            match sqlx::query(sql).bind(RETENTION_DAYS).execute(&pool).await {
                Ok(r) if r.rows_affected() > 0 => println!("visitor retention: {} rows pruned ({sql})", r.rows_affected()),
                Ok(_) => {}
                Err(e) => eprintln!("visitor retention failed: {e}"),
            }
        }
    }
}

// ── Stats ────────────────────────────────────────────────────────────────
// Every panel is shown twice, once per class: "people" are rows with
// is_bot = false, bots are is_bot = true. Rows logged before classification
// existed (is_bot NULL) count as neither. Both use the same window so the
// two tabs compare like for like.

const WINDOW: &str = "visited_at > NOW() - INTERVAL '30 days'";

async fn scalar(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap_or(0)
}

/// Rows of `label`, `reqs`, `ips` → `[{label, reqs, ips}]`.
async fn ranked(pool: &PgPool, sql: &str) -> Vec<Value> {
    sqlx::query(sql)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .iter()
        .map(|row| {
            json!({
                "label": row.try_get::<String, _>("label").unwrap_or_default(),
                "reqs": row.try_get::<i64, _>("reqs").unwrap_or(0),
                "ips": row.try_get::<i64, _>("ips").unwrap_or(0),
            })
        })
        .collect()
}

/// `label` grouped over one class's rows (`class` is a SQL predicate),
/// ordered by `order` (`ips` or `reqs`).
async fn ranked_by(pool: &PgPool, class: &str, label: &str, extra: &str, order: &str) -> Vec<Value> {
    ranked(
        pool,
        &format!(
            "SELECT {label} AS label, COUNT(*) AS reqs, COUNT(DISTINCT ip) AS ips
             FROM visitors WHERE {class} AND {WINDOW} {extra}
             GROUP BY 1 ORDER BY {order} DESC LIMIT 8"
        ),
    )
    .await
}

/// Map dots for one class, rounded to 0.1° so nearby visits share a dot.
async fn map_points(pool: &PgPool, class: &str) -> Vec<Value> {
    sqlx::query(&format!(
        "SELECT ROUND(lat::numeric, 1)::float8 as lat, ROUND(lon::numeric, 1)::float8 as lon
         FROM visitors
         WHERE {class} AND {WINDOW} AND lat IS NOT NULL AND lon IS NOT NULL
         GROUP BY ROUND(lat::numeric, 1), ROUND(lon::numeric, 1) LIMIT 500"
    ))
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .iter()
    .map(|row| {
        let lat = row.try_get::<f64, _>("lat").unwrap_or(0.0);
        let lon = row.try_get::<f64, _>("lon").unwrap_or(0.0);
        let (x, y) = map_percent(lon, lat);
        // x/y: CSS left/top on the world map, bound per dot via
        // fx-bind-attr="style.left=item:x style.top=item:y".
        json!({ "lat": lat, "lon": lon, "x": x, "y": y })
    })
    .collect()
}

const PEOPLE: &str = "is_bot = false";
const BOTS: &str = "is_bot";

pub fn fetch_visitor_stats(pool: &PgPool) -> Value {
    let result: Result<Value, sqlx::Error> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            // Fail fast (→ "connected": false) if the DB is unreachable.
            sqlx::query("SELECT 1").execute(pool).await?;

            let ips = |class| format!("SELECT COUNT(DISTINCT ip) FROM visitors WHERE {class} AND {WINDOW}");
            let countries = |class| {
                format!(
                    "SELECT COUNT(DISTINCT country_code) FROM visitors
                     WHERE {class} AND country_code IS NOT NULL AND {WINDOW}"
                )
            };
            let requests = |class| format!("SELECT COUNT(*) FROM visitors WHERE {class} AND {WINDOW}");

            let people = scalar(pool, &ips(PEOPLE)).await;
            let visits = scalar(
                pool,
                &format!(
                    "SELECT COUNT(*) FROM (SELECT DISTINCT ip, visited_at::date FROM visitors
                      WHERE {PEOPLE} AND {WINDOW}) sub"
                ),
            )
            .await;
            let human_countries = scalar(pool, &countries(PEOPLE)).await;
            let human_requests = scalar(pool, &requests(PEOPLE)).await;

            let bot_ips = scalar(pool, &ips(BOTS)).await;
            let bot_countries = scalar(pool, &countries(BOTS)).await;
            let bot_requests = scalar(pool, &requests(BOTS)).await;
            let probes = scalar(pool, &format!("SELECT COUNT(*) FROM visitors WHERE status = 404 AND {WINDOW}")).await;
            let bot_share = match bot_requests + human_requests {
                0 => "–".to_string(),
                total => format!("{}%", bot_requests * 100 / total),
            };

            let country = "COALESCE(country, 'Unknown')";
            let isp = "COALESCE(isp, 'Unknown')";
            let top_countries = ranked_by(pool, PEOPLE, country, "", "ips").await;
            let top_pages = ranked_by(pool, PEOPLE, "path", "", "ips").await;
            let human_networks = ranked_by(pool, PEOPLE, isp, "", "ips").await;
            let referrers = ranked_by(
                pool,
                PEOPLE,
                "referer",
                "AND referer IS NOT NULL AND referer NOT LIKE '%jaydanhoward.com%'",
                "ips",
            )
            .await;
            let bot_top_countries = ranked_by(pool, BOTS, country, "", "reqs").await;
            let top_probes = ranked_by(pool, "status = 404", "path", "", "reqs").await;
            let bot_networks = ranked_by(pool, BOTS, isp, "", "reqs").await;

            let recent_rows = sqlx::query(&format!(
                "SELECT country, city, path, visited_at FROM (
                     SELECT DISTINCT ON (ip) country, city, path, visited_at
                     FROM visitors WHERE {PEOPLE} AND {WINDOW}
                     ORDER BY ip, visited_at DESC
                 ) deduped ORDER BY visited_at DESC LIMIT 20"
            ))
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

            Ok(json!({
                "connected": true,
                "people": people,
                "visits": visits,
                "countries": human_countries,
                "top_countries": top_countries,
                "top_pages": top_pages,
                "human_networks": human_networks,
                "referrers": referrers,
                "recent_visits": recent_visits,
                "points": map_points(pool, PEOPLE).await,
                "bot_requests": bot_requests,
                "bot_ips": bot_ips,
                "bot_countries": bot_countries,
                "probes": probes,
                "bot_share": bot_share,
                "bot_top_countries": bot_top_countries,
                "top_probes": top_probes,
                "bot_networks": bot_networks,
                "bot_points": map_points(pool, BOTS).await,
            }))
        })
    });

    result.unwrap_or_else(|e| {
        json!({ "connected": false, "error": e.to_string(), "people": 0, "visits": 0, "countries": 0,
                "top_countries": [], "top_pages": [], "human_networks": [], "referrers": [],
                "recent_visits": [], "points": [], "bot_requests": 0, "bot_ips": 0, "bot_countries": 0,
                "probes": 0, "bot_share": "–", "bot_top_countries": [], "top_probes": [],
                "bot_networks": [], "bot_points": [] })
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

/// `project` as CSS percentages of the 360x180 map, e.g. `("50.00%", "25.00%")`.
fn map_percent(lon: f64, lat: f64) -> (String, String) {
    let (x, y) = project(lon, lat);
    (format!("{:.2}%", x / 360.0 * 100.0), format!("{:.2}%", y / 180.0 * 100.0))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_percent_corners_and_center() {
        assert_eq!(map_percent(0.0, 0.0), ("50.00%".into(), "50.00%".into()));
        assert_eq!(map_percent(-180.0, 90.0), ("0.00%".into(), "0.00%".into()));
        assert_eq!(map_percent(180.0, -90.0), ("100.00%".into(), "100.00%".into()));
    }

    #[test]
    fn logs_pages_and_probes_only() {
        assert!(should_log("/", 200));
        assert!(should_log("/robots.txt", 200));
        assert!(should_log("/.env", 404));
        assert!(should_log("/api/.env", 404));
        assert!(!should_log("/state", 200));
        assert!(!should_log("/events", 200));
        assert!(!should_log("/api/lighthouse", 200));
        assert!(!should_log("/pkg/foster_client.js", 200));
        assert!(!should_log("/photos/a.b/x.webp", 200));
    }

    #[test]
    fn classifies_bots() {
        let chrome = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";
        assert!(!looks_automated(Some(chrome), 200));
        assert!(looks_automated(Some(chrome), 404));
        assert!(looks_automated(None, 200));
        assert!(looks_automated(Some(""), 200));
        assert!(looks_automated(Some("curl/8.5.0"), 200));
        assert!(looks_automated(Some("Mozilla/5.0 (compatible; Googlebot/2.1)"), 200));
        assert!(looks_automated(Some("Mozilla/5.0 HeadlessChrome/130.0 Chrome-Lighthouse"), 200));
    }

    #[test]
    fn geo_budget_caps_per_minute_and_pauses() {
        let mut b = GeoBudget::new();
        let t0 = b.window_start;
        for _ in 0..GEO_LOOKUPS_PER_MIN {
            assert!(b.take(t0));
        }
        assert!(!b.take(t0));
        assert!(b.take(t0 + Duration::from_secs(61)));
        b.pause(Duration::from_secs(3600));
        assert!(!b.take(Instant::now() + Duration::from_secs(120)));
    }
}
