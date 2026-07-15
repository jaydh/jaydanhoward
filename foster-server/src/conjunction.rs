//! Real conjunction screening — Hoots altitude-band pre-filter, true SGP4
//! propagation, 288-step/5-minute rolling-24h window scan, ternary-search
//! TCA refinement, rayon-parallelized pair scanning. Ported near-verbatim
//! from the real `src/components/conjunction.rs`'s `screening` module (the
//! scientific core is what makes results real, per the migration plan).
//!
//! Deliberately NOT ported: the distributed chunk-claiming work queue
//! (`conjunction_chunks`, `FOR UPDATE SKIP LOCKED`) that lets multiple
//! replicas split the N² pair space without double-counting. This is a
//! single-process screening pass — correct and complete for one replica.
//! If/when the production Deployment's 3 replicas need to split the work
//! (rather than only one of them ever handling a screening request), the
//! chunk-claiming machinery is the piece to port next; it doesn't change
//! the correctness of the algorithm itself, only horizontal coordination.

use rayon::prelude::*;
use serde::Serialize;
use serde_json::Value;
use sgp4::{Constants, Elements};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;

const MU: f64 = 398_600.4418;
const EARTH_RADIUS: f64 = 6_371.0;
const J2000_UNIX_MS: f64 = 946_728_000_000.0;
const STEP_MINUTES: f64 = 5.0;
const STEPS: usize = 288;
const STEP_MS: f64 = STEP_MINUTES * 60_000.0;
const MISS_THRESHOLD_KM: f64 = 10.0;
const HOOTS_BUFFER_KM: f64 = 30.0;

#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum Screening {
    Idle,
    Running,
    Complete {
        total_pairs: usize,
        pairs_after_hoots: usize,
        events_found: usize,
        elapsed_ms: u64,
        events: Vec<ConjunctionEventOut>,
    },
    Failed {
        error: String,
    },
}

#[derive(Clone, Serialize)]
pub struct ConjunctionEventOut {
    pub sat_a: String,
    pub sat_b: String,
    pub tca_unix_ms: f64,
    pub miss_distance_km: f32,
    pub rel_velocity_km_s: f32,
}

pub type ScreeningState = Arc<Mutex<Screening>>;

pub fn initial_state() -> ScreeningState {
    Arc::new(Mutex::new(Screening::Idle))
}

#[derive(Clone)]
pub struct ConjunctionAppState {
    pub screening: ScreeningState,
    pub pool: PgPool,
}

fn altitude_band(line2: &str) -> (f64, f64) {
    let e: f64 = line2.get(26..33).map(|s| format!("0.{}", s.trim())).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let n_rev_per_day: f64 = line2.get(52..63).and_then(|s| s.trim().parse().ok()).unwrap_or(15.0);
    let n_rad_s = n_rev_per_day * 2.0 * std::f64::consts::PI / 86_400.0;
    let a = (MU / (n_rad_s * n_rad_s)).cbrt();
    (a * (1.0 - e) - EARTH_RADIUS, a * (1.0 + e) - EARTH_RADIUS)
}

fn hoots_pass(line2_a: &str, line2_b: &str) -> bool {
    if line2_a.len() < 63 || line2_b.len() < 63 {
        return false;
    }
    let (peri_a, apo_a) = altitude_band(line2_a);
    let (peri_b, apo_b) = altitude_band(line2_b);
    peri_a <= apo_b + HOOTS_BUFFER_KM && peri_b <= apo_a + HOOTS_BUFFER_KM
}

pub struct SatProp {
    pub name: String,
    constants: Constants,
    epoch_j2000_years: f64,
    pub line2: String,
}

unsafe impl Send for SatProp {}
unsafe impl Sync for SatProp {}

impl SatProp {
    pub fn new(name: &str, line1: &str, line2: &str) -> Option<Self> {
        let elements = Elements::from_tle(Some(name.to_string()), line1.as_bytes(), line2.as_bytes()).ok()?;
        let constants = Constants::from_elements(&elements).ok()?;
        Some(Self { name: name.to_string(), constants, epoch_j2000_years: elements.epoch(), line2: line2.to_string() })
    }

    fn eci_pos(&self, time_unix_ms: f64) -> Option<[f64; 3]> {
        let minutes_j2000 = (time_unix_ms - J2000_UNIX_MS) / 60_000.0;
        let epoch_minutes = self.epoch_j2000_years * 365.25 * 24.0 * 60.0;
        let tsince = minutes_j2000 - epoch_minutes;
        self.constants.propagate(sgp4::MinutesSinceEpoch(tsince)).ok().map(|p| p.position)
    }
}

#[inline]
fn dist(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn find_tca(a: &SatProp, b: &SatProp, mut lo: f64, mut hi: f64) -> f64 {
    for _ in 0..24 {
        let m1 = lo + (hi - lo) / 3.0;
        let m2 = hi - (hi - lo) / 3.0;
        let d1 = match (a.eci_pos(m1), b.eci_pos(m1)) { (Some(pa), Some(pb)) => dist(&pa, &pb), _ => f64::MAX };
        let d2 = match (a.eci_pos(m2), b.eci_pos(m2)) { (Some(pa), Some(pb)) => dist(&pa, &pb), _ => f64::MAX };
        if d1 < d2 { hi = m2 } else { lo = m1 }
    }
    (lo + hi) / 2.0
}

fn propagate_pair(pa: &SatProp, pb: &SatProp, now_unix_ms: f64) -> Vec<ConjunctionEventOut> {
    let mut pos_a = Vec::with_capacity(STEPS);
    let mut pos_b = Vec::with_capacity(STEPS);
    for i in 0..STEPS {
        let t = now_unix_ms + i as f64 * STEP_MS;
        match (pa.eci_pos(t), pb.eci_pos(t)) {
            (Some(a), Some(b)) => { pos_a.push(a); pos_b.push(b); }
            _ => { pos_a.push([f64::MAX, 0.0, 0.0]); pos_b.push([f64::MAX, 0.0, 0.0]); }
        }
    }
    let dists: Vec<f64> = pos_a.iter().zip(pos_b.iter()).map(|(a, b)| dist(a, b)).collect();
    let mut events = Vec::new();

    for i in 1..dists.len().saturating_sub(1) {
        if dists[i] < MISS_THRESHOLD_KM && dists[i] <= dists[i - 1] && dists[i] <= dists[i + 1] {
            let t_lo = now_unix_ms + (i as f64 - 1.0) * STEP_MS;
            let t_hi = now_unix_ms + (i as f64 + 1.0) * STEP_MS;
            let tca_ms = find_tca(pa, pb, t_lo, t_hi);
            let (tca_a, tca_b) = match (pa.eci_pos(tca_ms), pb.eci_pos(tca_ms)) { (Some(a), Some(b)) => (a, b), _ => continue };
            let miss_km = dist(&tca_a, &tca_b) as f32;

            const DT_MS: f64 = 30_000.0;
            let rel_vel = match (pa.eci_pos(tca_ms + DT_MS), pb.eci_pos(tca_ms + DT_MS), pa.eci_pos(tca_ms - DT_MS), pb.eci_pos(tca_ms - DT_MS)) {
                (Some(a1), Some(b1), Some(a0), Some(b0)) => {
                    let vx = ((a1[0] - b1[0]) - (a0[0] - b0[0])) / (2.0 * DT_MS / 1000.0);
                    let vy = ((a1[1] - b1[1]) - (a0[1] - b0[1])) / (2.0 * DT_MS / 1000.0);
                    let vz = ((a1[2] - b1[2]) - (a0[2] - b0[2])) / (2.0 * DT_MS / 1000.0);
                    (vx * vx + vy * vy + vz * vz).sqrt() as f32
                }
                _ => 0.0,
            };

            events.push(ConjunctionEventOut {
                sat_a: pa.name.clone(), sat_b: pb.name.clone(),
                tca_unix_ms: tca_ms, miss_distance_km: miss_km, rel_velocity_km_s: rel_vel,
            });
        }
    }
    events
}

fn fetch_tle_group_blocking(group: &str) -> Vec<(String, String, String)> {
    let url = format!("https://celestrak.org/NORAD/elements/gp.php?GROUP={group}&FORMAT=tle");
    let body = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; jaydanhoward-foster-migration)")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .and_then(|c| c.get(&url).send())
        .and_then(|r| r.text())
        .unwrap_or_default();
    body.lines()
        .collect::<Vec<_>>()
        .chunks(3)
        .filter(|c| c.len() == 3)
        .map(|c| (c[0].trim().to_string(), c[1].to_string(), c[2].to_string()))
        .collect()
}

/// Real screening pass: fetches real TLEs (stations + gps-ops + geo — same
/// groups the Satellites section uses), applies the Hoots pre-filter, then
/// rayon-parallel SGP4-propagates every surviving pair over a real 24h/5min
/// window with ternary-search TCA refinement. Persists to the real
/// conjunction_screenings/conjunction_events tables (single-process; see
/// module doc comment on the deferred distributed chunk-claiming).
fn run_screening_blocking(pool: PgPool) -> Screening {
    let started = SystemTime::now();

    let tles: Vec<(String, String, String)> = ["stations", "gps-ops", "geo"]
        .iter()
        .flat_map(|g| fetch_tle_group_blocking(g))
        .collect();

    let props: Vec<Option<SatProp>> = tles.iter().map(|(n, l1, l2)| SatProp::new(n, l1, l2)).collect();
    let n = props.len();
    let total_pairs: usize = (0..n).map(|i| n.saturating_sub(i + 1)).sum();
    let now_unix_ms = started.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as f64;

    // Single pass per anchor: count Hoots-surviving pairs and collect any
    // real close-approach events from those pairs, together.
    let per_anchor: Vec<(usize, Vec<ConjunctionEventOut>)> = (0..n)
        .into_par_iter()
        .map(|i| {
            let Some(pa) = &props[i] else { return (0, Vec::new()) };
            let mut hoots_count = 0usize;
            let mut events = Vec::new();
            for j in (i + 1)..n {
                let Some(pb) = &props[j] else { continue };
                if !hoots_pass(&pa.line2, &pb.line2) {
                    continue;
                }
                hoots_count += 1;
                events.extend(propagate_pair(pa, pb, now_unix_ms));
            }
            (hoots_count, events)
        })
        .collect();

    let pairs_after_hoots: usize = per_anchor.iter().map(|(c, _)| c).sum();
    let events: Vec<ConjunctionEventOut> = per_anchor.into_iter().flat_map(|(_, e)| e).collect();

    let elapsed_ms = started.elapsed().unwrap_or_default().as_millis() as u64;

    // Persist to the real schema (blocking call already off the async
    // runtime worker thread here, so a small nested block_on is fine —
    // same reasoning as cluster.rs/visitors.rs).
    let events_for_db = events.clone();
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "local-dev".to_string());
            let screening_id: Result<i64, sqlx::Error> = sqlx::query_scalar(
                "INSERT INTO conjunction_screenings (group_name, status, completed_at, total_pairs, pairs_after_hoots, events_found, elapsed_ms, calculated_by)
                 VALUES ($1, 'complete', NOW(), $2, $3, $4, $5, $6) RETURNING id",
            )
            .bind("stations+gps-ops+geo")
            .bind(total_pairs as i64)
            .bind(pairs_after_hoots as i64)
            .bind(events_for_db.len() as i32)
            .bind(elapsed_ms as i64)
            .bind(&hostname)
            .fetch_one(&pool)
            .await;

            if let Ok(id) = screening_id {
                for e in &events_for_db {
                    let _ = sqlx::query(
                        "INSERT INTO conjunction_events (screening_id, sat_a, sat_b, tca_unix_ms, miss_distance_km, rel_velocity_km_s)
                         VALUES ($1, $2, $3, $4, $5, $6)",
                    )
                    .bind(id)
                    .bind(&e.sat_a)
                    .bind(&e.sat_b)
                    .bind(e.tca_unix_ms)
                    .bind(e.miss_distance_km)
                    .bind(e.rel_velocity_km_s)
                    .execute(&pool)
                    .await;
                }
            }
        })
    });

    Screening::Complete { total_pairs, pairs_after_hoots, events_found: events.len(), elapsed_ms, events }
}

pub async fn get_screening(state: axum::extract::State<ConjunctionAppState>) -> axum::Json<Value> {
    let current = state.0.screening.lock().await.clone();
    // On a fresh process (nothing screened yet this run), fall back to the
    // last real completed screening from the DB rather than always
    // reporting "idle" until someone clicks the button again.
    if matches!(current, Screening::Idle) {
        let from_db = latest_from_db(&state.0.pool).await;
        if from_db["status"] == "complete" {
            return axum::Json(from_db);
        }
    }
    axum::Json(serde_json::to_value(current).unwrap_or(serde_json::json!({"status":"idle"})))
}

pub async fn start_screening(state: axum::extract::State<ConjunctionAppState>) -> axum::http::StatusCode {
    {
        let mut s = state.0.screening.lock().await;
        *s = Screening::Running;
    }
    let screening = state.0.screening.clone();
    let pool = state.0.pool.clone();
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || run_screening_blocking(pool)).await.unwrap_or_else(|e| Screening::Failed { error: e.to_string() });
        let mut s = screening.lock().await;
        *s = result;
    });
    axum::http::StatusCode::ACCEPTED
}

/// Real, previously-completed screenings from the DB (for initial page load
/// before anyone clicks "Screen").
pub async fn latest_from_db(pool: &PgPool) -> Value {
    let row = sqlx::query(
        "SELECT id, total_pairs, pairs_after_hoots, events_found, elapsed_ms FROM conjunction_screenings
         WHERE status = 'complete' ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    match row {
        Some(r) => {
            use sqlx::Row;
            let id: i64 = r.try_get("id").unwrap_or(0);
            let events = sqlx::query("SELECT sat_a, sat_b, tca_unix_ms, miss_distance_km, rel_velocity_km_s FROM conjunction_events WHERE screening_id = $1 ORDER BY tca_unix_ms LIMIT 20")
                .bind(id)
                .fetch_all(pool)
                .await
                .unwrap_or_default();
            let events: Vec<Value> = events.iter().map(|e| {
                serde_json::json!({
                    "sat_a": e.try_get::<String,_>("sat_a").unwrap_or_default(),
                    "sat_b": e.try_get::<String,_>("sat_b").unwrap_or_default(),
                    "miss_distance_km": e.try_get::<f32,_>("miss_distance_km").unwrap_or(0.0),
                })
            }).collect();
            serde_json::json!({
                "status": "complete",
                "total_pairs": r.try_get::<i64,_>("total_pairs").unwrap_or(0),
                "pairs_after_hoots": r.try_get::<i64,_>("pairs_after_hoots").unwrap_or(0),
                "events_found": r.try_get::<i32,_>("events_found").unwrap_or(0),
                "events": events,
            })
        }
        None => serde_json::json!({ "status": "idle" }),
    }
}
