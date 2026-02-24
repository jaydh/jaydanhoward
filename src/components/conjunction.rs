#![allow(clippy::all)]
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

// ──────────────────────────────────────────────────────────────
// Shared types (SSR + WASM)
// ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConjunctionEvent {
    pub sat_a: String,
    pub sat_b: String,
    /// Unix timestamp (ms) of closest approach
    pub tca_unix_ms: f64,
    pub miss_distance_km: f32,
    pub rel_velocity_km_s: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreeningStats {
    pub total_pairs: usize,
    pub pairs_after_hoots: usize,
    pub events_found: usize,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScreeningStatus {
    Idle,
    Running { started_unix_ms: f64 },
    Complete { stats: ScreeningStats },
    Failed(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConjunctionResult {
    pub group: String,
    pub status: ScreeningStatus,
    /// Sorted by tca_unix_ms ascending (may be partial during Running)
    pub events: Vec<ConjunctionEvent>,
}

// ──────────────────────────────────────────────────────────────
// In-memory cache (SSR only) — used for dev (no DB) and as live
// preview during screening on all deployments
// ──────────────────────────────────────────────────────────────

#[cfg(feature = "ssr")]
pub type ConjunctionCache =
    tokio::sync::RwLock<std::collections::HashMap<String, ConjunctionResult>>;

// ──────────────────────────────────────────────────────────────
// SSR-only: screening logic
// ──────────────────────────────────────────────────────────────

#[cfg(feature = "ssr")]
pub mod screening {
    use super::*;
    use crate::components::satellite_tracker::TleData;
    use rayon::prelude::*;
    use sgp4::{Constants, Elements};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::SystemTime;

    const MU: f64 = 398_600.4418; // km³/s²
    const EARTH_RADIUS: f64 = 6_371.0; // km
    const J2000_UNIX_MS: f64 = 946_728_000_000.0;
    const STEP_MINUTES: f64 = 5.0;
    const STEPS: usize = 288; // 24 h / 5 min
    const STEP_MS: f64 = STEP_MINUTES * 60_000.0;
    const MISS_THRESHOLD_KM: f64 = 10.0;
    const HOOTS_BUFFER_KM: f64 = 30.0;

    /// Parse perigee/apogee altitude (km) from TLE line 2 using the Hoots approximation.
    fn altitude_band(line2: &str) -> (f64, f64) {
        // Eccentricity: TLE cols 27-33 (1-indexed), i.e. bytes 26-32 (0-indexed); prepend "0."
        let e: f64 = line2
            .get(26..33)
            .map(|s| format!("0.{}", s.trim()))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);

        // Mean motion (rev/day): TLE cols 53-63 (1-indexed), i.e. bytes 52-62 (0-indexed)
        let n_rev_per_day: f64 = line2
            .get(52..63)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(15.0);

        let n_rad_s = n_rev_per_day * 2.0 * std::f64::consts::PI / 86_400.0;
        let a = (MU / (n_rad_s * n_rad_s)).cbrt();

        let perigee = a * (1.0 - e) - EARTH_RADIUS;
        let apogee = a * (1.0 + e) - EARTH_RADIUS;
        (perigee, apogee)
    }

    /// Returns true if the two orbits have overlapping altitude bands (Hoots filter).
    fn hoots_pass(a: &TleData, b: &TleData) -> bool {
        // Standard TLE line 2 is 69 characters; we need at least 63 for mean motion field.
        if a.line2.len() < 63 || b.line2.len() < 63 {
            return false;
        }
        let (peri_a, apo_a) = altitude_band(&a.line2);
        let (peri_b, apo_b) = altitude_band(&b.line2);
        peri_a <= apo_b + HOOTS_BUFFER_KM && peri_b <= apo_a + HOOTS_BUFFER_KM
    }

    /// Lightweight SGP4 propagator wrapper (ECI positions only).
    pub struct SatProp {
        pub name: String,
        constants: Constants,
        epoch_j2000_years: f64,
    }

    // Safety: Constants contains only f64 fields; it is Send + Sync.
    unsafe impl Send for SatProp {}
    unsafe impl Sync for SatProp {}

    impl SatProp {
        pub fn new(tle: &TleData) -> Option<Self> {
            let elements = Elements::from_tle(
                Some(tle.name.clone()),
                tle.line1.as_bytes(),
                tle.line2.as_bytes(),
            )
            .ok()?;
            let constants = Constants::from_elements(&elements).ok()?;
            let epoch_j2000_years = elements.epoch();
            Some(Self {
                name: tle.name.clone(),
                constants,
                epoch_j2000_years,
            })
        }

        /// ECI position (km) at the given Unix timestamp (ms).
        fn eci_pos(&self, time_unix_ms: f64) -> Option<[f64; 3]> {
            let minutes_j2000 = (time_unix_ms - J2000_UNIX_MS) / 60_000.0;
            let epoch_minutes = self.epoch_j2000_years * 365.25 * 24.0 * 60.0;
            let tsince = minutes_j2000 - epoch_minutes;
            self.constants
                .propagate(sgp4::MinutesSinceEpoch(tsince))
                .ok()
                .map(|p| p.position)
        }
    }

    #[inline]
    fn dist(a: &[f64; 3], b: &[f64; 3]) -> f64 {
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        let dz = a[2] - b[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Ternary search for TCA in [lo_ms, hi_ms] (minimise inter-satellite distance).
    fn find_tca(a: &SatProp, b: &SatProp, mut lo: f64, mut hi: f64) -> f64 {
        for _ in 0..24 {
            let m1 = lo + (hi - lo) / 3.0;
            let m2 = hi - (hi - lo) / 3.0;
            let d1 = match (a.eci_pos(m1), b.eci_pos(m1)) {
                (Some(pa), Some(pb)) => dist(&pa, &pb),
                _ => f64::MAX,
            };
            let d2 = match (a.eci_pos(m2), b.eci_pos(m2)) {
                (Some(pa), Some(pb)) => dist(&pa, &pb),
                _ => f64::MAX,
            };
            if d1 < d2 {
                hi = m2;
            } else {
                lo = m1;
            }
        }
        (lo + hi) / 2.0
    }

    /// Propagate a single pair over 24 h, returning any close-approach events.
    fn propagate_pair(
        pa: &SatProp,
        pb: &SatProp,
        now_unix_ms: f64,
    ) -> Vec<ConjunctionEvent> {
        let mut pos_a = Vec::with_capacity(STEPS);
        let mut pos_b = Vec::with_capacity(STEPS);

        for i in 0..STEPS {
            let t = now_unix_ms + i as f64 * STEP_MS;
            match (pa.eci_pos(t), pb.eci_pos(t)) {
                (Some(a), Some(b)) => {
                    pos_a.push(a);
                    pos_b.push(b);
                }
                _ => {
                    // Use sentinel so indices stay aligned
                    pos_a.push([f64::MAX, 0.0, 0.0]);
                    pos_b.push([f64::MAX, 0.0, 0.0]);
                }
            }
        }

        let dists: Vec<f64> = pos_a
            .iter()
            .zip(pos_b.iter())
            .map(|(a, b)| dist(a, b))
            .collect();

        let mut events = Vec::new();

        for i in 1..dists.len().saturating_sub(1) {
            // Local minimum below threshold
            if dists[i] < MISS_THRESHOLD_KM
                && dists[i] <= dists[i - 1]
                && dists[i] <= dists[i + 1]
            {
                let t_lo = now_unix_ms + (i as f64 - 1.0) * STEP_MS;
                let t_hi = now_unix_ms + (i as f64 + 1.0) * STEP_MS;
                let tca_ms = find_tca(pa, pb, t_lo, t_hi);

                let (tca_a, tca_b) = match (pa.eci_pos(tca_ms), pb.eci_pos(tca_ms)) {
                    (Some(a), Some(b)) => (a, b),
                    _ => continue,
                };
                let miss_km = dist(&tca_a, &tca_b) as f32;

                // Relative velocity via finite difference (±30 s)
                const DT_MS: f64 = 30_000.0;
                let rel_vel = match (
                    pa.eci_pos(tca_ms + DT_MS),
                    pb.eci_pos(tca_ms + DT_MS),
                    pa.eci_pos(tca_ms - DT_MS),
                    pb.eci_pos(tca_ms - DT_MS),
                ) {
                    (Some(a1), Some(b1), Some(a0), Some(b0)) => {
                        let vx = ((a1[0] - b1[0]) - (a0[0] - b0[0])) / (2.0 * DT_MS / 1000.0);
                        let vy = ((a1[1] - b1[1]) - (a0[1] - b0[1])) / (2.0 * DT_MS / 1000.0);
                        let vz = ((a1[2] - b1[2]) - (a0[2] - b0[2])) / (2.0 * DT_MS / 1000.0);
                        (vx * vx + vy * vy + vz * vz).sqrt() as f32
                    }
                    _ => 0.0,
                };

                events.push(ConjunctionEvent {
                    sat_a: pa.name.clone(),
                    sat_b: pb.name.clone(),
                    tca_unix_ms: tca_ms,
                    miss_distance_km: miss_km,
                    rel_velocity_km_s: rel_vel,
                });
            }
        }

        events
    }

    /// Screen all pairs in a TLE group for conjunctions over the next 24 h.
    ///
    /// Processes anchor satellites sequentially so partial results can be streamed
    /// via `tx` every `FLUSH_EVERY` anchors. Inner (j) pairs are parallelised with
    /// Rayon so all cores are kept busy throughout.
    pub fn screen_group(
        tles: &[TleData],
        tx: &tokio::sync::mpsc::UnboundedSender<Vec<ConjunctionEvent>>,
    ) -> ScreeningStats {
        const FLUSH_EVERY: usize = 50;

        let n = tles.len();
        let total_pairs = n * (n.saturating_sub(1)) / 2;

        // Pre-build all propagators once.
        let props: Vec<Option<SatProp>> = tles.iter().map(|t| SatProp::new(t)).collect();

        let pairs_after_hoots = AtomicUsize::new(0);
        let now_unix_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64;

        let mut batch: Vec<ConjunctionEvent> = Vec::new();
        let mut events_found = 0usize;

        for i in 0..n {
            // Inner pairs for this anchor, parallelised via Rayon
            let row_events: Vec<ConjunctionEvent> = (i + 1..n)
                .into_par_iter()
                .flat_map(|j| {
                    if !hoots_pass(&tles[i], &tles[j]) {
                        return vec![];
                    }
                    pairs_after_hoots.fetch_add(1, Ordering::Relaxed);
                    let pa = match props[i].as_ref() {
                        Some(p) => p,
                        None => return vec![],
                    };
                    let pb = match props[j].as_ref() {
                        Some(p) => p,
                        None => return vec![],
                    };
                    propagate_pair(pa, pb, now_unix_ms)
                })
                .collect();

            events_found += row_events.len();
            batch.extend(row_events);

            // Flush a chunk to the async receiver every FLUSH_EVERY anchors (or at end)
            if (i + 1) % FLUSH_EVERY == 0 || i == n - 1 {
                if !batch.is_empty() {
                    let _ = tx.send(std::mem::take(&mut batch));
                }
            }
        }

        ScreeningStats {
            total_pairs,
            pairs_after_hoots: pairs_after_hoots.load(Ordering::Relaxed),
            events_found,
            elapsed_ms: 0, // filled in by caller
        }
    }
}

// ──────────────────────────────────────────────────────────────
// screen_and_store: run screening, stream results to DB + cache
// ──────────────────────────────────────────────────────────────

#[cfg(feature = "ssr")]
pub async fn screen_and_store(
    pool: Option<actix_web::web::Data<sqlx::PgPool>>,
    cache: Option<actix_web::web::Data<ConjunctionCache>>,
    group: &str,
    tles: &[crate::components::satellite_tracker::TleData],
) {
    use std::time::Instant;
    use tokio::sync::mpsc;

    let started_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64;

    // Mark Running in in-memory cache immediately so clients see live status
    if let Some(ref c) = cache {
        let mut w = c.write().await;
        w.insert(
            group.to_string(),
            ConjunctionResult {
                group: group.to_string(),
                status: ScreeningStatus::Running { started_unix_ms: started_ms },
                events: vec![],
            },
        );
    }

    // Claim a DB slot if a pool is available (distributed lock)
    let screening_id: Option<i64> = if let Some(ref pool) = pool {
        match crate::db::start_conjunction_screening(pool, group).await {
            Ok(Some(id)) => Some(id),
            Ok(None) => {
                tracing::debug!(
                    "Conjunction screening for group={} already running on another replica, skipping",
                    group
                );
                // Remove the Running entry we just wrote — another replica has it
                if let Some(ref c) = cache {
                    c.write().await.remove(group);
                }
                return;
            }
            Err(e) => {
                tracing::error!("Failed to start screening record for group={}: {}", group, e);
                if let Some(ref c) = cache {
                    c.write().await.remove(group);
                }
                return;
            }
        }
    } else {
        None
    };

    tracing::info!(
        "Conjunction screening started: group={group} satellites={} screening_id={screening_id:?}",
        tles.len()
    );

    // Channel: screening thread sends event chunks, async task consumes them
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<ConjunctionEvent>>();
    let tles_owned = tles.to_vec();
    let t0 = Instant::now();

    let handle =
        tokio::task::spawn_blocking(move || screening::screen_group(&tles_owned, &tx));

    // Consume chunks as they arrive, updating DB and in-memory cache incrementally
    let mut all_events: Vec<ConjunctionEvent> = Vec::new();
    while let Some(chunk) = rx.recv().await {
        // Insert partial events to DB if we have a slot
        if let (Some(ref pool), Some(id)) = (&pool, screening_id) {
            if let Err(e) = crate::db::insert_conjunction_events(pool, id, &chunk).await {
                tracing::warn!("Failed to insert partial conjunction events: {}", e);
            }
        }

        all_events.extend(chunk);

        // Update in-memory cache with accumulated events (sorted)
        if let Some(ref c) = cache {
            let mut sorted = all_events.clone();
            sorted.sort_by(|a, b| {
                a.tca_unix_ms
                    .partial_cmp(&b.tca_unix_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut w = c.write().await;
            if let Some(result) = w.get_mut(group) {
                result.events = sorted;
            }
        }
    }

    // Channel closed — screening complete; collect final stats
    let elapsed_ms = t0.elapsed().as_millis() as u64;
    match handle.await {
        Ok(mut stats) => {
            stats.elapsed_ms = elapsed_ms;
            tracing::info!(
                "Conjunction screening complete: group={group} events={} pairs_screened={} elapsed_ms={}",
                stats.events_found,
                stats.pairs_after_hoots,
                stats.elapsed_ms
            );

            if let (Some(ref pool), Some(id)) = (&pool, screening_id) {
                if let Err(e) = crate::db::complete_conjunction_screening(
                    pool,
                    id,
                    stats.total_pairs as i64,
                    stats.pairs_after_hoots as i64,
                    stats.events_found as i32,
                    stats.elapsed_ms as i64,
                )
                .await
                {
                    tracing::error!("Failed to mark screening complete: {}", e);
                }
            }

            // Update in-memory cache to Complete
            if let Some(ref c) = cache {
                let mut w = c.write().await;
                if let Some(result) = w.get_mut(group) {
                    result.status = ScreeningStatus::Complete { stats };
                    // result.events already accumulated in the recv loop
                }
            }
        }
        Err(e) => {
            tracing::error!("Conjunction screening panicked: group={group} {e:?}");
            if let (Some(ref pool), Some(id)) = (&pool, screening_id) {
                let _ =
                    crate::db::fail_conjunction_screening(pool, id, &format!("{e:?}")).await;
            }
            if let Some(ref c) = cache {
                let mut w = c.write().await;
                if let Some(result) = w.get_mut(group) {
                    result.status = ScreeningStatus::Failed(format!("{e:?}"));
                }
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Server functions
// ──────────────────────────────────────────────────────────────

#[server(name = GetConjunctionStatus, prefix = "/api", endpoint = "get_conjunction_status")]
pub async fn get_conjunction_status(
    group: String,
) -> Result<ScreeningStatus, ServerFnError<String>> {
    use actix_web::web::Data;
    use leptos_actix::extract;
    use sqlx::PgPool;

    // Prefer DB when available
    if let Ok(pool) = extract::<Data<PgPool>>().await {
        let row = crate::db::get_latest_conjunction_screening(&pool, &group)
            .await
            .map_err(|e| ServerFnError::ServerError(format!("{e}")))?;

        return Ok(match row {
            None => ScreeningStatus::Idle,
            Some(r) => match r.status.as_str() {
                "running" => ScreeningStatus::Running {
                    started_unix_ms: r.started_at.timestamp_millis() as f64,
                },
                "complete" => ScreeningStatus::Complete {
                    stats: ScreeningStats {
                        total_pairs: r.total_pairs as usize,
                        pairs_after_hoots: r.pairs_after_hoots as usize,
                        events_found: r.events_found as usize,
                        elapsed_ms: r.elapsed_ms as u64,
                    },
                },
                "failed" => ScreeningStatus::Failed(r.error_msg.unwrap_or_default()),
                _ => ScreeningStatus::Idle,
            },
        });
    }

    // No DB: fall back to in-memory cache (dev mode)
    if let Ok(cache) = extract::<Data<ConjunctionCache>>().await {
        let r = cache.read().await;
        return Ok(match r.get(&group) {
            None => ScreeningStatus::Idle,
            Some(result) => result.status.clone(),
        });
    }

    Ok(ScreeningStatus::Idle)
}

#[server(name = GetConjunctions, prefix = "/api", endpoint = "get_conjunctions")]
pub async fn get_conjunctions(
    group: String,
) -> Result<Vec<ConjunctionEvent>, ServerFnError<String>> {
    use actix_web::web::Data;
    use leptos_actix::extract;
    use sqlx::PgPool;

    // Prefer DB when available (returns events for Running and Complete screenings)
    if let Ok(pool) = extract::<Data<PgPool>>().await {
        return crate::db::get_latest_conjunction_events(&pool, &group)
            .await
            .map_err(|e| ServerFnError::ServerError(format!("{e}")));
    }

    // No DB: fall back to in-memory cache (dev mode)
    if let Ok(cache) = extract::<Data<ConjunctionCache>>().await {
        let r = cache.read().await;
        return Ok(r.get(&group).map(|res| res.events.clone()).unwrap_or_default());
    }

    Ok(vec![])
}

/// Clear all results and kick off fresh screenings for every group.
///
/// Uses TLEs already held in the TleCache (populated at startup / on first TLE
/// fetch).  Groups with no cached TLEs are skipped.
#[server(name = RetriggerConjunction, prefix = "/api", endpoint = "retrigger_conjunction")]
pub async fn retrigger_conjunction() -> Result<(), ServerFnError<String>> {
    use actix_web::web::Data;
    use leptos_actix::extract;
    use sqlx::PgPool;

    const GROUPS: &[&str] = &["stations", "gps-ops", "visual", "active", "starlink"];

    let pool_opt = extract::<Data<PgPool>>().await.ok();
    let cache = extract::<Data<ConjunctionCache>>().await.ok();
    let tle_cache = extract::<Data<crate::components::satellite_tracker::TleCache>>()
        .await
        .map_err(|_| ServerFnError::ServerError("TLE cache not available".into()))?;

    // Cancel running DB screenings and clear in-memory cache for all groups
    for &group in GROUPS {
        if let Some(ref pool) = pool_opt {
            let _ = crate::db::cancel_running_conjunction_screening(pool, group).await;
        }
        if let Some(ref c) = cache {
            c.write().await.remove(group);
        }
    }

    // Spawn a fresh screening for each group that has cached TLEs
    for &group in GROUPS {
        let tles = {
            let r = tle_cache.read().await;
            r.get(group).map(|(_, d)| d.clone())
        };
        let Some(tles) = tles else {
            tracing::debug!("retrigger: no TLE cache for group={group}, skipping");
            continue;
        };

        let pool_clone = pool_opt.clone();
        let cache_clone = cache.clone();
        let group_str = group.to_string();
        tokio::spawn(async move {
            screen_and_store(pool_clone, cache_clone, &group_str, &tles).await;
        });
    }

    Ok(())
}

// ──────────────────────────────────────────────────────────────
// Leptos component
// ──────────────────────────────────────────────────────────────

#[component]
pub fn ConjunctionPanel(#[allow(unused_variables)] group: ReadSignal<String>) -> impl IntoView {
    // SSR: provide default signals for view! macro (never written to on server)
    #[cfg(feature = "ssr")]
    let (status, _set_status) = signal(Option::<ScreeningStatus>::None);
    #[cfg(feature = "ssr")]
    let (events, _set_events) = signal(Vec::<ConjunctionEvent>::new());

    // Client: mutable signals driven by polling
    #[cfg(not(feature = "ssr"))]
    let (status, set_status) = signal(Option::<ScreeningStatus>::None);
    #[cfg(not(feature = "ssr"))]
    let (events, set_events) = signal(Vec::<ConjunctionEvent>::new());
    #[cfg(not(feature = "ssr"))]
    let (loading_events, set_loading_events) = signal(false);
    // Set to true once we've loaded the final events for a completed screening
    #[cfg(not(feature = "ssr"))]
    let (events_loaded, set_events_loaded) = signal(false);

    // ── Client-side polling ──────────────────────────────────
    #[cfg(not(feature = "ssr"))]
    {
        use leptos::leptos_dom::helpers::set_interval_with_handle;
        use std::time::Duration;

        Effect::new(move |_| {
            let current_group = group.get();
            // Reset state when group changes
            set_status.set(None);
            set_events.set(Vec::new());
            set_loading_events.set(false);
            set_events_loaded.set(false);

            // Spawn an interval that polls status every 3 s
            let handle = set_interval_with_handle(
                move || {
                    let g = current_group.clone();
                    leptos::task::spawn_local(async move {
                        // Skip if an events fetch is already in flight
                        if loading_events.get_untracked() {
                            return;
                        }

                        match get_conjunction_status(g.clone()).await {
                            Ok(s) => {
                                let is_running = matches!(s, ScreeningStatus::Running { .. });
                                let is_complete =
                                    matches!(s, ScreeningStatus::Complete { .. });
                                set_status.set(Some(s));

                                // Fetch events while Running (live trickle) or once on Complete
                                let already_done = events_loaded.get_untracked();
                                if (is_running || (is_complete && !already_done)) {
                                    set_loading_events.set(true);
                                    match get_conjunctions(g).await {
                                        Ok(ev) => {
                                            set_events.set(ev);
                                            if is_complete {
                                                set_events_loaded.set(true);
                                            }
                                        }
                                        Err(e) => {
                                            web_sys::console::error_1(
                                                &format!("get_conjunctions error: {e:?}")
                                                    .into(),
                                            );
                                        }
                                    }
                                    set_loading_events.set(false);
                                }
                            }
                            Err(e) => {
                                web_sys::console::warn_1(
                                    &format!("get_conjunction_status error: {e:?}").into(),
                                );
                            }
                        }
                    });
                },
                Duration::from_secs(3),
            );

            // Cancel interval on cleanup
            if let Ok(h) = handle {
                on_cleanup(move || h.clear());
            }
        });
    }

    // SSR: placeholder retrigger handler
    #[cfg(feature = "ssr")]
    let on_retrigger = move |_: leptos::ev::MouseEvent| {};

    // Client: calls server fn, resets local state
    #[cfg(not(feature = "ssr"))]
    let on_retrigger = move |_: leptos::ev::MouseEvent| {
        set_status.set(None);
        set_events.set(Vec::new());
        set_loading_events.set(false);
        set_events_loaded.set(false);
        leptos::task::spawn_local(async move {
            if let Err(e) = retrigger_conjunction().await {
                web_sys::console::error_1(&format!("retrigger error: {e:?}").into());
            }
        });
    };

    view! {
        <div class="w-full">
            <div class="flex items-center justify-between mt-2">
                <ConjunctionStatusBadge status=status />
                <button
                    class="text-xs text-muted hover:text-foreground border border-border \
                           rounded px-2 py-1 transition-colors shrink-0"
                    on:click=on_retrigger
                >
                    "Recalculate"
                </button>
            </div>
            <Show when=move || !events.get().is_empty()>
                <ConjunctionTable events=events />
            </Show>
        </div>
    }
}

#[component]
fn ConjunctionStatusBadge(status: ReadSignal<Option<ScreeningStatus>>) -> impl IntoView {
    view! {
        <div class="text-sm text-muted mt-2">
            {move || match status.get() {
                None => view! { <span>"Conjunction screening: waiting for TLE data..."</span> }.into_any(),
                Some(ScreeningStatus::Idle) => view! { <span>"Conjunction screening: idle"</span> }.into_any(),
                Some(ScreeningStatus::Running { .. }) => {
                    view! {
                        <span class="flex items-center gap-2">
                            <span class="inline-block w-2 h-2 rounded-full bg-yellow-400 animate-pulse"></span>
                            "Conjunction screening in progress..."
                        </span>
                    }
                    .into_any()
                }
                Some(ScreeningStatus::Complete { stats }) => {
                    view! {
                        <span>
                            {format!(
                                "Screening complete — {} events found ({} pairs screened in {}ms)",
                                stats.events_found,
                                stats.pairs_after_hoots,
                                stats.elapsed_ms,
                            )}
                        </span>
                    }
                    .into_any()
                }
                Some(ScreeningStatus::Failed(msg)) => {
                    view! { <span class="text-red-500">{format!("Screening failed: {}", msg)}</span> }
                        .into_any()
                }
            }}
        </div>
    }
}

// ── Sort helpers ──────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum SortCol {
    Tca,
    SatA,
    SatB,
    Miss,
    Vel,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SortDir {
    Asc,
    Desc,
}

// ── Virtualized, sortable, filterable table ───────────────────

#[component]
fn ConjunctionTable(events: ReadSignal<Vec<ConjunctionEvent>>) -> impl IntoView {
    let (sort_col, set_sort_col) = signal(SortCol::Tca);
    let (sort_dir, set_sort_dir) = signal(SortDir::Asc);
    let (search, set_search) = signal(String::new());

    // Scroll offset — only ever written client-side, but both builds define it
    // so view! captures compile cleanly on SSR.
    #[cfg(feature = "ssr")]
    let (scroll_top, _set_scroll_top) = signal(0.0_f64);
    #[cfg(not(feature = "ssr"))]
    let (scroll_top, set_scroll_top) = signal(0.0_f64);

    // Physical row height (px) — must match the <tr> style below.
    const ROW_H: f64 = 32.0;
    // Visible viewport height (px) — must match the container style below.
    const CONTAINER_H: f64 = 380.0;
    // Extra rows to render above/below the visible window.
    const BUFFER: usize = 8;

    // Derived: filtered + sorted list.
    let processed = Memo::new(move |_| {
        let mut evts = events.get();
        let q = search.get().to_lowercase();
        if !q.is_empty() {
            evts.retain(|e| {
                e.sat_a.to_lowercase().contains(&q) || e.sat_b.to_lowercase().contains(&q)
            });
        }
        let col = sort_col.get();
        let dir = sort_dir.get();
        evts.sort_by(|a, b| {
            use std::cmp::Ordering::Equal;
            let ord = match col {
                SortCol::Tca => a.tca_unix_ms.partial_cmp(&b.tca_unix_ms).unwrap_or(Equal),
                SortCol::SatA => a.sat_a.cmp(&b.sat_a),
                SortCol::SatB => a.sat_b.cmp(&b.sat_b),
                SortCol::Miss => a
                    .miss_distance_km
                    .partial_cmp(&b.miss_distance_km)
                    .unwrap_or(Equal),
                SortCol::Vel => a
                    .rel_velocity_km_s
                    .partial_cmp(&b.rel_velocity_km_s)
                    .unwrap_or(Equal),
            };
            if dir == SortDir::Desc { ord.reverse() } else { ord }
        });
        evts
    });

    // Factory for sort-click handlers — each call returns a fresh closure that
    // captures the Copy signal handles, so we can generate one per column header.
    let make_sort_click = move |col: SortCol| {
        move |_: leptos::ev::MouseEvent| {
            if sort_col.get_untracked() == col {
                set_sort_dir
                    .update(|d| *d = if *d == SortDir::Asc { SortDir::Desc } else { SortDir::Asc });
            } else {
                set_sort_col.set(col);
                set_sort_dir.set(SortDir::Asc);
            }
        }
    };

    // Factory for reactive sort-arrow labels.
    let make_sort_arrow = move |col: SortCol| {
        move || -> &'static str {
            if sort_col.get() == col {
                if sort_dir.get() == SortDir::Asc { " ▲" } else { " ▼" }
            } else {
                " ⇅"
            }
        }
    };

    // Virtual window: derive the slice of rows to actually render + padding heights.
    let virtual_window = Memo::new(move |_| {
        let all = processed.get();
        let n = all.len();
        let top = scroll_top.get();
        let start = ((top / ROW_H) as usize).saturating_sub(BUFFER);
        let end = (((top + CONTAINER_H) / ROW_H) as usize + BUFFER).min(n);
        let pad_top = (start as f64 * ROW_H) as u32;
        let pad_bot = (n.saturating_sub(end) as f64 * ROW_H) as u32;
        let slice = if start < n { all[start..end].to_vec() } else { vec![] };
        (pad_top, pad_bot, slice, n)
    });

    let th = "py-2 px-3 text-left text-xs font-semibold text-muted \
              cursor-pointer select-none whitespace-nowrap \
              hover:text-foreground transition-colors";

    view! {
        <div class="mt-3 flex flex-col gap-2">

            // ── Search + count ──────────────────────────────
            <div class="flex items-center gap-3">
                <input
                    type="text"
                    placeholder="Search by satellite name…"
                    class="flex-1 bg-surface border border-border rounded-md px-3 py-1.5 \
                           text-sm placeholder:text-muted \
                           focus:outline-none focus:ring-1 focus:ring-border"
                    prop:value=move || search.get()
                    on:input=move |e| set_search.set(event_target_value(&e))
                />
                <span class="text-xs text-muted whitespace-nowrap shrink-0">
                    {move || {
                        let (_, _, _, n) = virtual_window.get();
                        let total = events.get().len();
                        if n == total { format!("{n} events") }
                        else { format!("{n} / {total}") }
                    }}
                </span>
            </div>

            // ── Scrollable virtualized table ─────────────────
            <div
                class="overflow-y-auto border border-border rounded-lg"
                style="height: 380px;"
                on:scroll=move |e| {
                    let _ = &e; // keep `e` live on SSR where the body below is compiled out
                    #[cfg(not(feature = "ssr"))]
                    {
                        use wasm_bindgen::JsCast;
                        if let Some(el) = e
                            .target()
                            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                        {
                            set_scroll_top.set(el.scroll_top() as f64);
                        }
                    }
                }
            >
                <table class="w-full text-sm border-collapse">

                    // Sticky header row
                    <thead class="sticky top-0 bg-gray z-10 border-b-2 border-border">
                        <tr>
                            <th class=th on:click=make_sort_click(SortCol::Tca)>
                                "TCA" {make_sort_arrow(SortCol::Tca)}
                            </th>
                            <th class=th on:click=make_sort_click(SortCol::SatA)>
                                "Satellite A" {make_sort_arrow(SortCol::SatA)}
                            </th>
                            <th class=th on:click=make_sort_click(SortCol::SatB)>
                                "Satellite B" {make_sort_arrow(SortCol::SatB)}
                            </th>
                            <th class=th on:click=make_sort_click(SortCol::Miss)>
                                "Miss km" {make_sort_arrow(SortCol::Miss)}
                            </th>
                            <th class=th on:click=make_sort_click(SortCol::Vel)>
                                "Rel vel km/s" {make_sort_arrow(SortCol::Vel)}
                            </th>
                        </tr>
                    </thead>

                    <tbody>
                        // Top spacer
                        <tr style=move || {
                            let h = virtual_window.get().0;
                            if h > 0 { format!("height:{h}px") } else { "display:none".into() }
                        }>
                            <td colspan="5"></td>
                        </tr>

                        // Visible rows only
                        <For
                            each=move || virtual_window.get().2
                            key=|e| {
                                format!("{}-{}-{}", e.sat_a, e.sat_b, e.tca_unix_ms as u64)
                            }
                            children=|e| view! { <ConjunctionRow event=e /> }
                        />

                        // Bottom spacer
                        <tr style=move || {
                            let h = virtual_window.get().1;
                            if h > 0 { format!("height:{h}px") } else { "display:none".into() }
                        }>
                            <td colspan="5"></td>
                        </tr>
                    </tbody>
                </table>
            </div>
        </div>
    }
}

#[component]
fn ConjunctionRow(event: ConjunctionEvent) -> impl IntoView {
    let tca_rel = {
        #[cfg(not(feature = "ssr"))]
        {
            let now_ms = js_sys::Date::now();
            let delta_s = ((event.tca_unix_ms - now_ms) / 1000.0).max(0.0) as u64;
            let h = delta_s / 3600;
            let m = (delta_s % 3600) / 60;
            format!("in {h}h {m:02}m")
        }
        #[cfg(feature = "ssr")]
        {
            let _ = event.tca_unix_ms; // suppress unused
            String::new()
        }
    };

    view! {
        <tr
            class="border-b border-border hover:bg-surface-alt transition-colors"
            style="height:32px"
        >
            <td class="px-3 font-mono text-xs whitespace-nowrap">{tca_rel}</td>
            <td class="px-3 font-mono text-xs whitespace-nowrap">{event.sat_a.clone()}</td>
            <td class="px-3 font-mono text-xs whitespace-nowrap">{event.sat_b.clone()}</td>
            <td class="px-3 font-mono text-xs whitespace-nowrap">
                {format!("{:.3}", event.miss_distance_km)}
            </td>
            <td class="px-3 font-mono text-xs whitespace-nowrap">
                {format!("{:.2}", event.rel_velocity_km_s)}
            </td>
        </tr>
    }
}
