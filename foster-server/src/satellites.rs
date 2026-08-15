//! Real 3D satellite tracking — ports the real site's
//! `satellite_calculations.rs` (real `sgp4` propagation, real ECI→render
//! coordinate swap) and the data half of `satellite_tracker.rs` (real
//! CelesTrak "active" TLE group — ~16k objects, same group the real site
//! fetches — real 288-step/5-minute rolling-24h time grid, real per-
//! satellite altitude/inclination classification, real Astranis pinning).
//!
//! One necessary architectural adaptation: the real site runs `sgp4`
//! *inside the browser* (compiled to WASM), re-propagating every satellite
//! on every `requestAnimationFrame`. Foster has no custom per-app WASM —
//! only its own fixed `foster-client` runtime ships to the browser — so
//! that can't be reused directly. Instead this module runs the exact same
//! `sgp4` crate and math *once per server tick* (shared across every
//! connected client, not per-tab), caches the resulting position snapshot,
//! and serves it from a plain polled route
//! (`static/satellites.js`, same "poll a hand-rolled axum route
//! independently of Foster's SSE" shape as `conjunction.js`). The client
//! interpolates between the two most recent real snapshots for smooth
//! motion instead of recomputing SGP4 itself. Real data, real orbital
//! mechanics, real time grid — only *where* the propagation loop runs
//! changed, forced by the framework, not a fidelity cut.
//!
//! The actual WebGL2 rendering pipeline (shaders, sphere/equator/pole
//! geometry, camera matrices, draw calls) is a faithful line-for-line port
//! of `satellite_renderer.rs` into `static/satellites.js` — that part needed
//! no architectural adaptation at all, just a language change.

use chrono::{DateTime, Utc};
use rayon::prelude::*;
use serde_json::{json, Value};
use sgp4::{Constants, Elements};
use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

const EARTH_RADIUS_KM: f64 = 6371.0;
const J2000_UNIX_MS: f64 = 946_728_000_000.0;
const STEP_MINUTES: f64 = 5.0;
const STEPS: usize = 288;
const STEP_MS: f64 = STEP_MINUTES * 60_000.0;
const TLE_TTL: Duration = Duration::from_secs(6 * 3600);
const TICK: Duration = Duration::from_millis(1000);

// Astranis satellite pinning/highlighting (same NORAD IDs and rationale as
// the real satellite_renderer.rs/satellite_tracker.rs) is applied entirely
// client-side in static/satellites.js, since it only affects rendering
// (color + orbit-filter bypass), not the real position data this module
// computes and serves.

struct RealSat {
    norad_id: u32,
    constants: Constants,
    epoch_j2000_years: f64,
    inclination_deg: f64,
}

// sgp4::Constants holds no interior mutability or non-Send state; safe to
// share across the rayon pool the same way conjunction.rs's SatProp does.
unsafe impl Send for RealSat {}
unsafe impl Sync for RealSat {}

impl RealSat {
    fn from_tle(line1: &str, line2: &str) -> Option<Self> {
        let elements = Elements::from_tle(None, line1.as_bytes(), line2.as_bytes()).ok()?;
        let constants = Constants::from_elements(&elements).ok()?;
        let norad_id = line1.get(2..7).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        Some(Self {
            norad_id,
            constants,
            epoch_j2000_years: elements.epoch(),
            inclination_deg: elements.inclination,
        })
    }

    /// Real position at an absolute unix-ms timestamp, in the render's
    /// coordinate convention (x,z equatorial / y polar, Earth-radius units)
    /// — identical swap and scale to the real `satellite_calculations.rs`.
    fn position_at(&self, time_ms: f64) -> Option<Value> {
        let minutes_j2000 = (time_ms - J2000_UNIX_MS) / 60_000.0;
        let epoch_minutes = self.epoch_j2000_years * 365.25 * 24.0 * 60.0;
        let tsince = minutes_j2000 - epoch_minutes;
        let prediction = self.constants.propagate(sgp4::MinutesSinceEpoch(tsince)).ok()?;
        let scale = 1.0 / EARTH_RADIUS_KM;
        let p = prediction.position;
        let distance_from_center = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        let altitude_km = distance_from_center - EARTH_RADIUS_KM;
        Some(json!({
            "x": p[0] * scale,
            "y": p[2] * scale,
            "z": -(p[1] * scale),
            "altitude_km": altitude_km,
            "inclination_deg": self.inclination_deg,
            "norad_id": self.norad_id,
        }))
    }
}

fn fetch_active_group_blocking() -> Vec<(String, String)> {
    // CelesTrak throttles repeat downloads of an unchanged group from the
    // same IP (serves a 403 + plain-text notice instead of TLE data) —
    // handy for local dev iteration without waiting out the throttle
    // window each time. Unset in production; the real fetch below is used
    // there, same as every other group fetch in this migration.
    let body = if let Ok(path) = std::env::var("SATELLITES_TLE_FIXTURE") {
        std::fs::read_to_string(path).unwrap_or_default()
    } else {
        let url = "https://celestrak.org/NORAD/elements/gp.php?GROUP=active&FORMAT=tle";
        reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (compatible; jaydanhoward-foster-migration)")
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .and_then(|c| c.get(url).send())
            .and_then(|r| r.text())
            .unwrap_or_default()
    };
    body.lines()
        .collect::<Vec<_>>()
        .chunks(3)
        .filter(|c| c.len() == 3 && c[1].trim_start().starts_with("1 ") && c[2].trim_start().starts_with("2 "))
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .collect()
}

const TLE_CACHE_GROUP: &str = "active";

struct Cache {
    /// When these TLEs were actually fetched from CelesTrak — a real wall
    /// clock time (not process-local), so a cache loaded from Postgres on
    /// a fresh pod correctly inherits its true age instead of restarting
    /// the TLE_TTL countdown from "now" every restart.
    fetched_at: DateTime<Utc>,
    sats: Vec<RealSat>,
    /// Fixed 288-point/5-minute grid covering the trailing 24h as of the
    /// last TLE refresh — same shape as the real site's `time_points`,
    /// just re-anchored on each refresh instead of once per page load.
    time_points: Vec<f64>,
}

fn build_cache_from_tles(tles: Vec<(String, String)>, fetched_at: DateTime<Utc>) -> Cache {
    let sats: Vec<RealSat> = tles
        .par_iter()
        .filter_map(|(l1, l2)| RealSat::from_tle(l1, l2))
        .collect();

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64;
    let start_time = now_ms - 24.0 * 60.0 * 60.0 * 1000.0;
    let time_points: Vec<f64> = (0..STEPS).map(|i| start_time + i as f64 * STEP_MS).collect();

    Cache { fetched_at, sats, time_points }
}

/// Try to atomically claim the right to do the live CelesTrak refetch for
/// this minute bucket. Returns true if this replica won the race — same
/// INSERT ON CONFLICT DO NOTHING pattern as spike_claims/cluster_audit_claims.
async fn try_claim_tle_refresh(pool: &PgPool) -> bool {
    let result = sqlx::query(
        "INSERT INTO tle_refresh_claims (bucket) \
         VALUES (date_trunc('minute', NOW())) \
         ON CONFLICT DO NOTHING",
    )
    .execute(pool)
    .await;

    matches!(result, Ok(r) if r.rows_affected() == 1)
}

async fn load_tle_cache(pool: &PgPool, group: &str) -> Option<(Vec<(String, String)>, DateTime<Utc>)> {
    let row: (Value, DateTime<Utc>) = sqlx::query_as(
        "SELECT satellites, fetched_at FROM tle_cache WHERE group_name = $1",
    )
    .bind(group)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()?;
    let tles: Vec<(String, String)> = serde_json::from_value(row.0).ok()?;
    Some((tles, row.1))
}

async fn save_tle_cache(pool: &PgPool, group: &str, tles: &[(String, String)], fetched_at: DateTime<Utc>) {
    let json = serde_json::to_value(tles).unwrap_or(Value::Null);
    let _ = sqlx::query(
        "INSERT INTO tle_cache (group_name, satellites, fetched_at) VALUES ($1, $2, $3)
         ON CONFLICT (group_name) DO UPDATE SET satellites = EXCLUDED.satellites, fetched_at = EXCLUDED.fetched_at",
    )
    .bind(group)
    .bind(json)
    .bind(fetched_at)
    .execute(pool)
    .await;
}

/// Live CelesTrak fetch + persist to Postgres (best-effort — the in-memory
/// cache still works fine without a pool for local dev), so the *next*
/// process restart can skip the ~44s CelesTrak round trip entirely by
/// reading this back via `load_tle_cache` instead.
async fn refresh_and_cache(pool: &Option<PgPool>) -> Cache {
    let tles = tokio::task::spawn_blocking(fetch_active_group_blocking).await.unwrap();
    let fetched_at = Utc::now();
    if let Some(pool) = pool {
        save_tle_cache(pool, TLE_CACHE_GROUP, &tles, fetched_at).await;
    }
    tokio::task::spawn_blocking(move || build_cache_from_tles(tles, fetched_at)).await.unwrap()
}

pub struct SatellitesRuntime {
    pub running: Arc<AtomicBool>,
    pub steps_per_tick: Arc<AtomicU32>,
    pub snapshot: Arc<RwLock<Value>>,
}

impl SatellitesRuntime {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(true)),
            // 12 steps/tick * 5 sim-min/step @ 1 tick/sec = 1 sim-hour/sec,
            // matching the real site's default ("1.0h/s" shown pre-hydration).
            steps_per_tick: Arc::new(AtomicU32::new(12)),
            snapshot: Arc::new(RwLock::new(json!({
                "time_ms": 0.0,
                "count": 0,
                "positions": [],
            }))),
        }
    }
}

impl Default for SatellitesRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Spawns the shared background propagation loop: refreshes the real TLE
/// set every 6h (same TTL as the real site), advances a real time index
/// through the 288-point grid when running, and re-propagates every
/// satellite's real position (rayon-parallel, same as conjunction.rs) once
/// per tick — one computation shared by every connected client, not one per
/// browser tab.
pub fn spawn_background_loop(runtime: Arc<SatellitesRuntime>, pool: Option<PgPool>) {
    tokio::spawn(async move {
        // A fresh pod otherwise blocks every /api/satellites response on an
        // empty snapshot for as long as the live CelesTrak fetch takes
        // (~44s observed in production) before its first real tick — real
        // user impact, not just a cold-start curiosity (it chained straight
        // into the page's LCP). Loading the last-known-good TLE set from
        // Postgres first means a restart only pays that cost if genuinely
        // nothing's cached yet or the cache has aged past TLE_TTL.
        let mut cache = match &pool {
            Some(p) => match load_tle_cache(p, TLE_CACHE_GROUP).await {
                Some((tles, fetched_at)) => {
                    tokio::task::spawn_blocking(move || build_cache_from_tles(tles, fetched_at)).await.unwrap()
                }
                None => refresh_and_cache(&pool).await,
            },
            None => refresh_and_cache(&pool).await,
        };
        let mut index: usize = 0;
        let mut ticker = tokio::time::interval(TICK);

        loop {
            ticker.tick().await;

            if Utc::now().signed_duration_since(cache.fetched_at).to_std().unwrap_or(TLE_TTL) > TLE_TTL {
                // 3 replicas share this one Postgres-backed cache with no
                // coordination otherwise — without this claim, all 3 would
                // independently notice the same staleness and each pay the
                // ~44s live CelesTrak fetch, and any client polling
                // /api/satellites during that window could land on a
                // mid-refresh replica and see an empty response (the exact
                // stall this cache was built to prevent in the first
                // place — just recurring every TLE_TTL instead of every
                // pod restart). Only the replica that wins the claim
                // refetches; the others wait for it to land in Postgres
                // and re-read from there instead.
                if let Some(p) = &pool {
                    if try_claim_tle_refresh(p).await {
                        cache = refresh_and_cache(&pool).await;
                        index = 0;
                    } else {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        if let Some((tles, fetched_at)) = load_tle_cache(p, TLE_CACHE_GROUP).await {
                            cache = tokio::task::spawn_blocking(move || build_cache_from_tles(tles, fetched_at))
                                .await
                                .unwrap();
                            index = 0;
                        }
                    }
                } else {
                    cache = refresh_and_cache(&pool).await;
                    index = 0;
                }
            }

            if runtime.running.load(Ordering::Relaxed) && !cache.time_points.is_empty() {
                let steps = runtime.steps_per_tick.load(Ordering::Relaxed).max(1) as usize;
                index = (index + steps) % cache.time_points.len();
            }

            let time_ms = cache.time_points.get(index).copied().unwrap_or(0.0);
            let sats = &cache.sats;
            let positions: Vec<Value> = tokio::task::block_in_place(|| {
                sats.par_iter().filter_map(|s| s.position_at(time_ms)).collect()
            });

            let snap = json!({
                "time_ms": time_ms,
                "count": positions.len(),
                "positions": positions,
            });
            *runtime.snapshot.write().await = snap;
        }
    });
}

pub async fn get_positions(
    state: axum::extract::State<Arc<SatellitesRuntime>>,
) -> axum::Json<Value> {
    axum::Json(state.0.snapshot.read().await.clone())
}
