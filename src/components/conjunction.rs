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
    /// Hostname of the node that calculated this event
    pub calculated_by: String,
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
    Running { started_unix_ms: f64, total_pairs: usize, pairs_propagated: usize },
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

/// Orbital elements and derived parameters for one satellite.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrbitalParams {
    pub name: String,
    pub semi_major_axis_km: f32,
    pub period_minutes: f32,
    pub eccentricity: f32,
    pub inclination_deg: f32,
    pub raan_deg: f32,
    pub arg_perigee_deg: f32,
    pub perigee_alt_km: f32,
    pub apogee_alt_km: f32,
}

/// Full detail for one conjunction pair, computed on demand.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConjunctionDetail {
    pub sat_a: OrbitalParams,
    pub sat_b: OrbitalParams,
    /// Inter-satellite distance (km) at each 5-minute step over 24 h (288 values).
    /// Values > 200 km are clamped to 200 for display purposes.
    pub distance_profile_km: Vec<f32>,
    /// 2-D positions projected onto sat-A's orbital plane (km), 288 entries.
    /// Coordinate origin is Earth's centre; x = first position vector direction.
    pub proj_a: Vec<[f32; 2]>,
    pub proj_b: Vec<[f32; 2]>,
    /// Projected positions at TCA.
    pub tca_proj_a: [f32; 2],
    pub tca_proj_b: [f32; 2],
    pub hoots_overlap: bool,
    pub tca_unix_ms: f64,
    pub miss_distance_km: f32,
    pub rel_velocity_km_s: f32,
    pub tle_a_line1: String,
    pub tle_a_line2: String,
    pub tle_b_line1: String,
    pub tle_b_line2: String,
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
                    calculated_by: String::new(), // filled in by screen_and_store
                });
            }
        }

        events
    }

    /// Screen satellite pairs for conjunctions over the next 24 h.
    ///
    /// Only pairs where the "A" satellite index is in `[sat_start, sat_end)` are
    /// screened, so callers can split the TLE list into non-overlapping chunks and
    /// process them in parallel across pods without double-counting any pair.
    /// Pass `sat_start = 0, sat_end = tles.len()` for a full single-pod screening.
    ///
    /// Results are streamed via `tx` every `FLUSH_EVERY` anchors as
    /// `(cumulative_pairs_after_hoots, event_batch)`.
    pub fn screen_group(
        tles: &[TleData],
        tx: &tokio::sync::mpsc::UnboundedSender<(usize, Vec<ConjunctionEvent>)>,
        sat_start: usize,
        sat_end: usize,
    ) -> ScreeningStats {
        const FLUSH_EVERY: usize = 50;

        let n = tles.len();
        let end = sat_end.min(n);
        let total_pairs: usize = if sat_start < end {
            (sat_start..end).map(|i| n.saturating_sub(i + 1)).sum()
        } else {
            0
        };

        // Pre-build all propagators once (full array needed for the inner j loop).
        let props: Vec<Option<SatProp>> = tles.iter().map(|t| SatProp::new(t)).collect();

        let pairs_after_hoots = AtomicUsize::new(0);
        let now_unix_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64;

        let mut batch: Vec<ConjunctionEvent> = Vec::new();
        let mut events_found = 0usize;

        for i in sat_start..end {
            // Inner pairs for this anchor, parallelised via Rayon.
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

            // Flush every FLUSH_EVERY anchors within this chunk, or at the last anchor.
            let anchors_done = i - sat_start + 1;
            if anchors_done % FLUSH_EVERY == 0 || i == end - 1 {
                let cumul_pairs = pairs_after_hoots.load(Ordering::Relaxed);
                let _ = tx.send((cumul_pairs, std::mem::take(&mut batch)));
            }
        }

        ScreeningStats {
            total_pairs,
            pairs_after_hoots: pairs_after_hoots.load(Ordering::Relaxed),
            events_found,
            elapsed_ms: 0, // filled in by caller
        }
    }

    /// Parse classical orbital elements from a TLE.
    pub fn orbital_params(tle: &TleData) -> Option<super::OrbitalParams> {
        let elements = sgp4::Elements::from_tle(
            Some(tle.name.clone()),
            tle.line1.as_bytes(),
            tle.line2.as_bytes(),
        )
        .ok()?;
        let n_rad_s = elements.mean_motion * 2.0 * std::f64::consts::PI / 86_400.0;
        let a = (MU / (n_rad_s * n_rad_s)).cbrt();
        let e = elements.eccentricity;
        Some(super::OrbitalParams {
            name: tle.name.clone(),
            semi_major_axis_km: a as f32,
            period_minutes: (1440.0 / elements.mean_motion) as f32,
            eccentricity: e as f32,
            inclination_deg: elements.inclination as f32,
            raan_deg: elements.right_ascension as f32,
            arg_perigee_deg: elements.argument_of_perigee as f32,
            perigee_alt_km: (a * (1.0 - e) - EARTH_RADIUS) as f32,
            apogee_alt_km: (a * (1.0 + e) - EARTH_RADIUS) as f32,
        })
    }

    /// Re-propagate a specific pair and return full detail for the UI.
    pub fn compute_pair_detail(
        tle_a: &TleData,
        tle_b: &TleData,
        now_unix_ms: f64,
    ) -> Option<super::ConjunctionDetail> {
        let sat_a = orbital_params(tle_a)?;
        let sat_b = orbital_params(tle_b)?;
        let hoots_overlap = hoots_pass(tle_a, tle_b);
        let pa = SatProp::new(tle_a)?;
        let pb = SatProp::new(tle_b)?;

        // Collect ECI positions and build distance profile simultaneously.
        let mut pos_a_eci: Vec<[f64; 3]> = Vec::with_capacity(STEPS);
        let mut pos_b_eci: Vec<[f64; 3]> = Vec::with_capacity(STEPS);
        let mut distance_profile_km: Vec<f32> = Vec::with_capacity(STEPS);

        for i in 0..STEPS {
            let t = now_unix_ms + i as f64 * STEP_MS;
            match (pa.eci_pos(t), pb.eci_pos(t)) {
                (Some(a), Some(b)) => {
                    distance_profile_km.push(dist(&a, &b).min(200.0) as f32);
                    pos_a_eci.push(a);
                    pos_b_eci.push(b);
                }
                _ => {
                    distance_profile_km.push(200.0);
                    pos_a_eci.push([0.0, 0.0, 0.0]);
                    pos_b_eci.push([0.0, 0.0, 0.0]);
                }
            }
        }

        // Find TCA: ternary-search around the minimum in the profile.
        let min_idx = distance_profile_km
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        let t_lo = now_unix_ms + (min_idx.saturating_sub(1) as f64) * STEP_MS;
        let t_hi = now_unix_ms + ((min_idx + 1).min(STEPS - 1) as f64) * STEP_MS;
        let tca_ms = find_tca(&pa, &pb, t_lo, t_hi);

        let (tca_a, tca_b) = match (pa.eci_pos(tca_ms), pb.eci_pos(tca_ms)) {
            (Some(a), Some(b)) => (a, b),
            _ => return None,
        };
        let miss_km = dist(&tca_a, &tca_b) as f32;

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

        // ── Orbital-plane projection ──────────────────────────────────
        // Orthonormal basis for sat-A's orbital plane:
        //   e1 = unit radial at step 0
        //   e3 = plane normal = normalize(e1 × pos_a[STEPS/2])
        //   e2 = e3 × e1  (in-plane, 90° from e1)
        #[inline]
        fn norm3(v: [f64; 3]) -> [f64; 3] {
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if len < 1e-10 { [1.0, 0.0, 0.0] } else { [v[0] / len, v[1] / len, v[2] / len] }
        }
        #[inline]
        fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
            [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
        }
        #[inline]
        fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
            a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
        }

        let e1 = norm3(pos_a_eci[0]);
        let mid = pos_a_eci.get(STEPS / 2).copied().unwrap_or([0.0, 1.0, 0.0]);
        let e3 = norm3(cross3(e1, mid));
        let e2 = cross3(e3, e1);

        let project = |p: [f64; 3]| -> [f32; 2] {
            [dot3(p, e1) as f32, dot3(p, e2) as f32]
        };

        let proj_a: Vec<[f32; 2]> = pos_a_eci.iter().map(|&p| project(p)).collect();
        let proj_b: Vec<[f32; 2]> = pos_b_eci.iter().map(|&p| project(p)).collect();
        let tca_proj_a = project(tca_a);
        let tca_proj_b = project(tca_b);

        Some(super::ConjunctionDetail {
            sat_a,
            sat_b,
            distance_profile_km,
            proj_a,
            proj_b,
            tca_proj_a,
            tca_proj_b,
            hoots_overlap,
            tca_unix_ms: tca_ms,
            miss_distance_km: miss_km,
            rel_velocity_km_s: rel_vel,
            tle_a_line1: tle_a.line1.clone(),
            tle_a_line2: tle_a.line2.clone(),
            tle_b_line1: tle_b.line1.clone(),
            tle_b_line2: tle_b.line2.clone(),
        })
    }
}

// ──────────────────────────────────────────────────────────────
// screen_and_store: run screening, stream results to DB + cache
// ──────────────────────────────────────────────────────────────

#[cfg(feature = "ssr")]
pub async fn screen_and_store(
    pool: Option<std::sync::Arc<sqlx::PgPool>>,
    cache: Option<std::sync::Arc<ConjunctionCache>>,
    group: &str,
    tles: &[crate::components::satellite_tracker::TleData],
    // Some(id) = slot already claimed by caller (startup path, claim-first).
    // None = let this function attempt the claim (manual retrigger path).
    pre_claimed_id: Option<i64>,
) {
    use std::time::Instant;
    use tokio::sync::mpsc;

    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());

    let started_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64;

    let n = tles.len();
    let total_pairs = n * n.saturating_sub(1) / 2;

    // Mark Running in in-memory cache immediately so clients see live status
    if let Some(ref c) = cache {
        let mut w = c.write().await;
        w.insert(
            group.to_string(),
            ConjunctionResult {
                group: group.to_string(),
                status: ScreeningStatus::Running {
                    started_unix_ms: started_ms,
                    total_pairs,
                    pairs_propagated: 0,
                },
                events: vec![],
            },
        );
    }

    // Claim a DB slot if a pool is available (distributed lock).
    // If a pre_claimed_id was supplied by the caller, use it directly — the slot is
    // already held and we skip the INSERT to avoid a spurious conflict.
    let screening_id: Option<i64> = if let Some(id) = pre_claimed_id {
        Some(id)
    } else if let Some(ref pool) = pool {
        match crate::db::start_conjunction_screening(pool, group, &hostname, total_pairs as i64).await {
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

    // If we pre-claimed with total_pairs=0, patch it now that we know the real count.
    if pre_claimed_id.is_some() {
        if let (Some(ref pool), Some(id)) = (&pool, screening_id) {
            if let Err(e) = crate::db::update_conjunction_total_pairs(pool, id, total_pairs as i64).await {
                tracing::warn!("Failed to update total_pairs for screening_id={id}: {e}");
            }
        }
    }

    tracing::info!(
        "Conjunction screening started: group={group} satellites={} screening_id={screening_id:?}",
        tles.len()
    );

    // Channel: screening thread sends (cumulative pairs propagated, event chunk)
    let (tx, mut rx) = mpsc::unbounded_channel::<(usize, Vec<ConjunctionEvent>)>();
    let tles_owned = tles.to_vec();
    let t0 = Instant::now();

    let n = tles_owned.len();
    let handle =
        tokio::task::spawn_blocking(move || screening::screen_group(&tles_owned, &tx, 0, n));

    // Consume chunks as they arrive, updating DB and in-memory cache incrementally
    let mut all_events: Vec<ConjunctionEvent> = Vec::new();
    while let Some((pairs_propagated, mut chunk)) = rx.recv().await {
        // Stamp the node hostname on every event
        for e in &mut chunk {
            e.calculated_by = hostname.clone();
        }

        // Update pairs_propagated in DB so the status poll can show live progress
        if let (Some(ref pool), Some(id)) = (&pool, screening_id) {
            if let Err(e) = crate::db::update_conjunction_pairs_propagated(
                pool,
                id,
                pairs_propagated as i64,
            )
            .await
            {
                tracing::warn!("Failed to update pairs propagated: {}", e);
            }
        }

        // Insert partial events to DB if we have a slot
        if let (Some(ref pool), Some(id)) = (&pool, screening_id) {
            if !chunk.is_empty() {
                if let Err(e) = crate::db::insert_conjunction_events(pool, id, &chunk).await {
                    tracing::warn!("Failed to insert partial conjunction events: {}", e);
                }
            }
        }

        all_events.extend(chunk);

        // Update in-memory cache with accumulated events (sorted) and live pair count
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
                if let ScreeningStatus::Running { pairs_propagated: ref mut pp, .. } =
                    result.status
                {
                    *pp = pairs_propagated;
                }
            }
        }
    }

    // Channel closed — screening complete; collect final stats
    let elapsed_ms = t0.elapsed().as_millis() as u64;
    match handle.await {
        Ok(mut stats) => {
            stats.elapsed_ms = elapsed_ms;
            tracing::info!(
                "Conjunction screening complete: group={group} events={} pairs_screened={} elapsed={}",
                stats.events_found,
                stats.pairs_after_hoots,
                fmt_duration(stats.elapsed_ms)
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
// screen_chunk: screen one DB-claimed satellite range
// ──────────────────────────────────────────────────────────────

#[cfg(feature = "ssr")]
pub async fn screen_chunk(
    pool: Option<std::sync::Arc<sqlx::PgPool>>,
    chunk: crate::db::ChunkInfo,
    tles: &[crate::components::satellite_tracker::TleData],
) {
    use std::time::Instant;
    use tokio::sync::mpsc;

    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
    let t0 = Instant::now();
    let chunk_id = chunk.chunk_id;
    let screening_id = chunk.screening_id;
    let sat_start = chunk.sat_start;
    let sat_end = chunk.sat_end;
    let group = chunk.group_name.clone();

    tracing::info!(
        "Chunk started: group={group} chunk={chunk_id} sats={sat_start}..{sat_end}"
    );

    let (tx, mut rx) = mpsc::unbounded_channel::<(usize, Vec<ConjunctionEvent>)>();
    let tles_owned = tles.to_vec();

    let handle = tokio::task::spawn_blocking(move || {
        screening::screen_group(&tles_owned, &tx, sat_start, sat_end)
    });

    let mut all_events: Vec<ConjunctionEvent> = Vec::new();
    let mut last_pairs: usize = 0;

    while let Some((cumul_pairs, mut batch)) = rx.recv().await {
        for e in &mut batch {
            e.calculated_by = hostname.clone();
        }

        // Increment live progress on the parent screening (atomic add — safe for concurrent pods).
        if let Some(ref p) = pool {
            let delta = cumul_pairs.saturating_sub(last_pairs) as i64;
            if delta > 0 {
                if let Err(e) =
                    crate::db::increment_conjunction_pairs_propagated(p, screening_id, delta).await
                {
                    tracing::warn!("Progress update failed for chunk={chunk_id}: {e}");
                }
            }
        }
        last_pairs = cumul_pairs;

        // Stream events into DB immediately.
        if let Some(ref p) = pool {
            if !batch.is_empty() {
                if let Err(e) =
                    crate::db::insert_conjunction_events(p, screening_id, &batch).await
                {
                    tracing::warn!("Event insert failed for chunk={chunk_id}: {e}");
                }
            }
        }

        all_events.extend(batch);
    }

    match handle.await {
        Ok(stats) => {
            let elapsed = t0.elapsed().as_millis() as i64;
            tracing::info!(
                "Chunk complete: group={group} chunk={chunk_id} \
                 pairs={} events={} elapsed={elapsed}ms",
                stats.pairs_after_hoots,
                all_events.len(),
            );
            if let Some(ref p) = pool {
                match crate::db::complete_chunk(
                    p,
                    chunk_id,
                    screening_id,
                    stats.pairs_after_hoots as i64,
                    all_events.len() as i32,
                    elapsed,
                )
                .await
                {
                    Ok(true) => tracing::info!(
                        "Screening complete (last chunk): group={group} screening={screening_id}"
                    ),
                    Ok(false) => {}
                    Err(e) => tracing::error!("complete_chunk failed for {chunk_id}: {e}"),
                }
            }
        }
        Err(e) => {
            tracing::error!("Chunk panicked: group={group} chunk={chunk_id}: {e:?}");
            if let Some(ref p) = pool {
                let _ = crate::db::fail_chunk(p, chunk_id, screening_id, "panic in screen_group")
                    .await;
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
    use axum::extract::Extension;
    use leptos_axum::extract;
    use sqlx::PgPool;
    use std::sync::Arc;

    // Prefer DB when available
    if let Some(pool) = extract::<Extension<Option<Arc<PgPool>>>>().await.ok().and_then(|e| e.0) {
        let row = crate::db::get_latest_conjunction_screening(&pool, &group)
            .await
            .map_err(|e| ServerFnError::ServerError(format!("{e}")))?;

        return Ok(match row {
            None => ScreeningStatus::Idle,
            Some(r) => match r.status.as_str() {
                "running" => ScreeningStatus::Running {
                    started_unix_ms: r.started_at.timestamp_millis() as f64,
                    total_pairs: r.total_pairs as usize,
                    pairs_propagated: r.pairs_after_hoots as usize,
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
    if let Ok(cache) = extract::<Extension<Arc<ConjunctionCache>>>().await {
        let r = cache.0.read().await;
        return Ok(match r.get(&group) {
            None => ScreeningStatus::Idle,
            Some(result) => match &result.status {
                ScreeningStatus::Running { started_unix_ms, total_pairs, pairs_propagated } => {
                    ScreeningStatus::Running {
                        started_unix_ms: *started_unix_ms,
                        total_pairs: *total_pairs,
                        pairs_propagated: *pairs_propagated,
                    }
                }
                other => other.clone(),
            },
        });
    }

    Ok(ScreeningStatus::Idle)
}

#[server(name = GetConjunctions, prefix = "/api", endpoint = "get_conjunctions")]
pub async fn get_conjunctions(
    group: String,
) -> Result<Vec<ConjunctionEvent>, ServerFnError<String>> {
    use axum::extract::Extension;
    use leptos_axum::extract;
    use sqlx::PgPool;
    use std::sync::Arc;

    // Prefer DB when available (returns events for Running and Complete screenings)
    if let Some(pool) = extract::<Extension<Option<Arc<PgPool>>>>().await.ok().and_then(|e| e.0) {
        return crate::db::get_latest_conjunction_events(&pool, &group)
            .await
            .map_err(|e| ServerFnError::ServerError(format!("{e}")));
    }

    // No DB: fall back to in-memory cache (dev mode)
    if let Ok(cache) = extract::<Extension<Arc<ConjunctionCache>>>().await {
        let r = cache.0.read().await;
        return Ok(r.get(&group).map(|res| res.events.clone()).unwrap_or_default());
    }

    Ok(vec![])
}

/// Re-propagate a specific pair and return full orbital detail for display.
#[server(name = GetConjunctionDetail, prefix = "/api", endpoint = "get_conjunction_detail")]
pub async fn get_conjunction_detail(
    sat_a_name: String,
    sat_b_name: String,
) -> Result<Option<ConjunctionDetail>, ServerFnError<String>> {
    use axum::extract::Extension;
    use leptos_axum::extract;
    use std::sync::Arc;
    use std::time::SystemTime;

    let tle_cache = extract::<Extension<Arc<crate::components::satellite_tracker::TleCache>>>()
        .await
        .map_err(|e| ServerFnError::ServerError(format!("{e}")))?
        .0;

    let (tle_a, tle_b) = {
        let read = tle_cache.read().await;
        let find = |name: &str| -> Option<crate::components::satellite_tracker::TleData> {
            read.values().find_map(|(_, tles)| tles.iter().find(|t| t.name == name).cloned())
        };
        (find(&sat_a_name), find(&sat_b_name))
    };

    let (Some(tle_a), Some(tle_b)) = (tle_a, tle_b) else {
        return Ok(None);
    };

    let now_unix_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64;

    Ok(screening::compute_pair_detail(&tle_a, &tle_b, now_unix_ms))
}

/// Clear all results and kick off fresh screenings for every group.
///
/// Uses TLEs already held in the TleCache (populated at startup / on first TLE
/// fetch).  Groups with no cached TLEs are skipped.
#[server(name = RetriggerConjunction, prefix = "/api", endpoint = "retrigger_conjunction")]
pub async fn retrigger_conjunction() -> Result<(), ServerFnError<String>> {
    use axum::extract::Extension;
    use leptos_axum::extract;
    use sqlx::PgPool;
    use std::sync::Arc;

    const GROUPS: &[&str] = &["stations", "gps-ops", "visual", "active", "starlink"];

    let pool_opt = extract::<Extension<Option<Arc<PgPool>>>>().await.ok().and_then(|e| e.0);
    let cache = extract::<Extension<Arc<ConjunctionCache>>>().await.ok().map(|e| e.0);
    let tle_cache = extract::<Extension<Arc<crate::components::satellite_tracker::TleCache>>>()
        .await
        .map_err(|_| ServerFnError::ServerError("TLE cache not available".into()))?
        .0;

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
            screen_and_store(pool_clone, cache_clone, &group_str, &tles, None).await;
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

            // Spawn an interval that polls status every 3 s
            let handle = set_interval_with_handle(
                move || {
                    let g = current_group.clone();
                    leptos::task::spawn_local(async move {
                        match get_conjunction_status(g.clone()).await {
                            Ok(s) => {
                                let is_running =
                                    matches!(s, ScreeningStatus::Running { .. });
                                let is_complete =
                                    matches!(s, ScreeningStatus::Complete { .. });
                                let is_failed =
                                    matches!(s, ScreeningStatus::Failed(_));
                                set_status.set(Some(s));

                                // Fetch events:
                                //   • Always during Running (live trickle).
                                //   • Once on Complete/Failed if we have no events yet
                                //     (initial page load or after a retrigger reset).
                                //   • Never re-fetch once events are present —
                                //     avoids the repeated set_events that causes flashing.
                                let need_events = is_running
                                    || ((is_complete || is_failed)
                                        && events.get_untracked().is_empty());

                                if need_events && !loading_events.get_untracked() {
                                    set_loading_events.set(true);
                                    match get_conjunctions(g).await {
                                        Ok(ev) => set_events.set(ev),
                                        Err(e) => web_sys::console::error_1(
                                            &format!("get_conjunctions error: {e:?}").into(),
                                        ),
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
            <Show when=move || {
                matches!(
                    status.get(),
                    Some(ScreeningStatus::Running { .. } | ScreeningStatus::Complete { .. })
                ) || !events.get().is_empty()
            }>
                <ConjunctionTable events=events />
            </Show>
        </div>
    }
}

// ── Stat formatters ───────────────────────────────────────────

fn fmt_count(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn fmt_duration(ms: u64) -> String {
    if ms >= 60_000 {
        let m = ms / 60_000;
        let s = (ms % 60_000) / 1_000;
        format!("{m}m {s:02}s")
    } else if ms >= 1_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        format!("{ms}ms")
    }
}

#[component]
fn ConjunctionStatusBadge(status: ReadSignal<Option<ScreeningStatus>>) -> impl IntoView {
    let chip = "inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs \
                bg-gray border border-border font-mono";

    view! {
        <div class="text-sm text-muted flex flex-col gap-1.5">
            // ── Status line ──────────────────────────────────
            {move || match status.get() {
                None => view! {
                    <span>"Conjunction screening: waiting for TLE data..."</span>
                }.into_any(),
                Some(ScreeningStatus::Idle) => view! {
                    <span>"Conjunction screening: idle"</span>
                }.into_any(),
                Some(ScreeningStatus::Running { .. }) => view! {
                    <span class="flex items-center gap-2">
                        <span class="inline-block w-2 h-2 rounded-full bg-yellow-400 animate-pulse"></span>
                        "Screening in progress…"
                    </span>
                }.into_any(),
                Some(ScreeningStatus::Complete { .. }) => view! {
                    <span class="flex items-center gap-1.5">
                        <span class="inline-block w-2 h-2 rounded-full bg-green-400"></span>
                        "Screening complete"
                    </span>
                }.into_any(),
                Some(ScreeningStatus::Failed(msg)) => view! {
                    <span class="text-red-500">{format!("Screening failed: {}", msg)}</span>
                }.into_any(),
            }}

            // ── Stats chips (Running + Complete) ─────────────
            {move || match status.get() {
                Some(ScreeningStatus::Running { total_pairs, pairs_propagated, .. }) => view! {
                    <div class="flex flex-wrap gap-1.5">
                        <span class=chip>"pairs " {fmt_count(total_pairs)}</span>
                        {(pairs_propagated > 0).then(|| view! {
                            <span class=chip>"propagated " {fmt_count(pairs_propagated)}</span>
                        })}
                    </div>
                }.into_any(),
                Some(ScreeningStatus::Complete { stats }) => {
                    let hoots_pct = if stats.total_pairs > 0 {
                        let eliminated = stats.total_pairs.saturating_sub(stats.pairs_after_hoots);
                        format!("{:.1}%", eliminated as f64 / stats.total_pairs as f64 * 100.0)
                    } else {
                        "—".to_string()
                    };
                    view! {
                        <div class="flex flex-wrap gap-1.5">
                            <span class=chip>"pairs " {fmt_count(stats.total_pairs)}</span>
                            <span class=chip>"hoots ↓" {hoots_pct}</span>
                            <span class=chip>"propagated " {fmt_count(stats.pairs_after_hoots)}</span>
                            <span class=chip>"events " {fmt_count(stats.events_found)}</span>
                            <span class=chip>"time " {fmt_duration(stats.elapsed_ms)}</span>
                        </div>
                    }.into_any()
                },
                _ => view! { <span></span> }.into_any(),
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
    Node,
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

    // Selected pair for the detail panel.
    #[cfg(feature = "ssr")]
    let (selected, set_selected) = signal(Option::<ConjunctionEvent>::None);
    #[cfg(not(feature = "ssr"))]
    let (selected, set_selected) = signal(Option::<ConjunctionEvent>::None);

    // Hovered pair for the info bar.
    #[cfg(feature = "ssr")]
    let (hovered_event, set_hovered_event) = signal(Option::<ConjunctionEvent>::None);
    #[cfg(not(feature = "ssr"))]
    let (hovered_event, set_hovered_event) = signal(Option::<ConjunctionEvent>::None);

    // Detail fetch state — only used client-side.
    #[cfg(feature = "ssr")]
    let (detail, _set_detail) = signal(Option::<ConjunctionDetail>::None);
    #[cfg(feature = "ssr")]
    let (detail_loading, _set_detail_loading) = signal(false);
    #[cfg(not(feature = "ssr"))]
    let (detail, set_detail) = signal(Option::<ConjunctionDetail>::None);
    #[cfg(not(feature = "ssr"))]
    let (detail_loading, set_detail_loading) = signal(false);

    // Fetch detail when selected changes — skip if the same pair is already loaded.
    #[cfg(not(feature = "ssr"))]
    Effect::new(move |_| {
        let Some(ev) = selected.get() else { return };

        // If we already have detail for this exact pair, don't re-propagate.
        let already_loaded = detail.get_untracked().map_or(false, |d| {
            d.sat_a.name == ev.sat_a && d.sat_b.name == ev.sat_b
        });
        if already_loaded {
            return;
        }

        set_detail.set(None);
        set_detail_loading.set(true);
        leptos::task::spawn_local(async move {
            match get_conjunction_detail(ev.sat_a.clone(), ev.sat_b.clone()).await {
                Ok(Some(d)) => set_detail.set(Some(d)),
                Ok(None) => {}
                Err(e) => web_sys::console::warn_1(
                    &format!("conjunction detail error: {e:?}").into(),
                ),
            }
            set_detail_loading.set(false);
        });
    });

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
                SortCol::Node => a.calculated_by.cmp(&b.calculated_by),
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

            // ── Focused-pair view (when a row is selected) ──────────
            <Show when=move || selected.get().is_some()>
                {move || selected.get().map(|ev| {
                    let tca_label = {
                        #[cfg(not(feature = "ssr"))]
                        {
                            let now_ms = js_sys::Date::now();
                            let delta_s = ((ev.tca_unix_ms - now_ms) / 1000.0).max(0.0) as u64;
                            let h = delta_s / 3600;
                            let m = (delta_s % 3600) / 60;
                            format!("TCA in {h}h {m:02}m")
                        }
                        #[cfg(feature = "ssr")]
                        { String::new() }
                    };
                    view! {
                        <div class="flex items-center gap-3 px-1 py-1">
                            <button
                                class="text-xs text-muted hover:text-foreground transition-colors flex items-center gap-1"
                                on:click=move |_| {
                                    #[cfg(not(feature = "ssr"))]
                                    set_selected.set(None);
                                }
                            >
                                "← all pairs"
                            </button>
                            <span class="text-charcoal-lighter text-xs">"|"</span>
                            <span class="font-mono text-xs font-semibold text-charcoal">
                                {ev.sat_a.clone()} " ↔ " {ev.sat_b.clone()}
                            </span>
                            <span class="ml-auto font-mono text-xs text-muted">
                                {format!("{:.3} km  •  {:.2} km/s  •  {}", ev.miss_distance_km, ev.rel_velocity_km_s, tca_label)}
                            </span>
                        </div>
                    }
                })}
            </Show>

            // ── Table view (when nothing selected) ──────────────────
            <Show when=move || selected.get().is_none()>

            // Search + count
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

            // Hover info bar
            <div class="h-7 flex items-center px-3 border border-border rounded-md bg-surface text-xs">
                {move || match hovered_event.get() {
                    None => view! {
                        <span class="text-muted">"Hover a row for a quick summary — click to focus"</span>
                    }.into_any(),
                    Some(ev) => {
                        let tca_label = {
                            #[cfg(not(feature = "ssr"))]
                            {
                                let now_ms = js_sys::Date::now();
                                let delta_s = ((ev.tca_unix_ms - now_ms) / 1000.0).max(0.0) as u64;
                                let h = delta_s / 3600;
                                let m = (delta_s % 3600) / 60;
                                format!("TCA in {h}h {m:02}m")
                            }
                            #[cfg(feature = "ssr")]
                            { String::new() }
                        };
                        view! {
                            <span class="flex gap-4 items-center w-full font-mono">
                                <span class="text-charcoal font-semibold">{ev.sat_a.clone()} " ↔ " {ev.sat_b.clone()}</span>
                                <span class="text-muted">"miss " <span class="text-charcoal">{format!("{:.3}", ev.miss_distance_km)}</span> " km"</span>
                                <span class="text-muted">"vel " <span class="text-charcoal">{format!("{:.2}", ev.rel_velocity_km_s)}</span> " km/s"</span>
                                <span class="text-muted ml-auto">{tca_label}</span>
                            </span>
                        }.into_any()
                    }
                }}
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
                            <th class=th on:click=make_sort_click(SortCol::Node)>
                                "Node" {make_sort_arrow(SortCol::Node)}
                            </th>
                        </tr>
                    </thead>

                    <tbody>
                        // Top spacer
                        <tr style=move || {
                            let h = virtual_window.get().0;
                            if h > 0 { format!("height:{h}px") } else { "display:none".into() }
                        }>
                            <td colspan="6"></td>
                        </tr>

                        // Visible rows only
                        <For
                            each=move || virtual_window.get().2
                            key=|e| {
                                format!("{}-{}-{}", e.sat_a, e.sat_b, e.tca_unix_ms as u64)
                            }
                            children=move |e| view! {
                                <ConjunctionRow
                                    event=e
                                    selected=selected
                                    set_selected=set_selected
                                    set_hovered=set_hovered_event
                                />
                            }
                        />

                        // Bottom spacer
                        <tr style=move || {
                            let h = virtual_window.get().1;
                            if h > 0 { format!("height:{h}px") } else { "display:none".into() }
                        }>
                            <td colspan="6"></td>
                        </tr>
                    </tbody>
                </table>
            </div>

            </Show> // end table-only Show

            // ── Detail panel (shown when a pair is selected) ──
            <Show when=move || selected.get().is_some()>
                <ConjunctionDetailPanel
                    selected=selected
                    detail=detail
                    loading=detail_loading
                    on_close=move |_| {
                        #[cfg(not(feature = "ssr"))]
                        set_selected.set(None);
                    }
                />
            </Show>
        </div>
    }
}

#[component]
fn ConjunctionRow(
    event: ConjunctionEvent,
    selected: ReadSignal<Option<ConjunctionEvent>>,
    set_selected: WriteSignal<Option<ConjunctionEvent>>,
    set_hovered: WriteSignal<Option<ConjunctionEvent>>,
) -> impl IntoView {
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
            let _ = event.tca_unix_ms;
            String::new()
        }
    };

    let ev = event.clone();
    let ev_hover = event.clone();
    let is_selected = {
        let ev_key = (event.sat_a.clone(), event.sat_b.clone(), event.tca_unix_ms as u64);
        move || {
            selected.get().map_or(false, |s| {
                (s.sat_a.clone(), s.sat_b.clone(), s.tca_unix_ms as u64) == ev_key
            })
        }
    };

    view! {
        <tr
            class=move || {
                let base = "border-b border-border transition-colors cursor-pointer";
                if is_selected() { format!("{base} bg-surface-alt") }
                else { format!("{base} hover:bg-surface-alt/60") }
            }
            style="height:32px"
            on:mouseenter=move |_| set_hovered.set(Some(ev_hover.clone()))
            on:mouseleave=move |_| set_hovered.set(None)
            on:click=move |_| {
                let key = (ev.sat_a.clone(), ev.sat_b.clone(), ev.tca_unix_ms as u64);
                set_selected.update(|s| {
                    let already = s.as_ref().map_or(false, |x| {
                        (x.sat_a.clone(), x.sat_b.clone(), x.tca_unix_ms as u64) == key
                    });
                    *s = if already { None } else { Some(ev.clone()) };
                });
            }
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
            <td class="px-3 font-mono text-xs whitespace-nowrap text-muted">
                {event.calculated_by.clone()}
            </td>
        </tr>
    }
}

// ── Detail panel ──────────────────────────────────────────────

#[component]
fn ConjunctionDetailPanel(
    selected: ReadSignal<Option<ConjunctionEvent>>,
    detail: ReadSignal<Option<ConjunctionDetail>>,
    loading: ReadSignal<bool>,
    on_close: impl Fn(leptos::ev::MouseEvent) + 'static,
) -> impl IntoView {
    let param_row = |name: &'static str, a: String, b: String| {
        view! {
            <tr class="border-b border-border/40">
                <td class="py-1 pr-3 text-xs text-muted whitespace-nowrap">{name}</td>
                <td class="py-1 pr-4 text-xs font-mono whitespace-nowrap">{a}</td>
                <td class="py-1 text-xs font-mono whitespace-nowrap">{b}</td>
            </tr>
        }
    };

    // ── Orbit pan / zoom state ────────────────────────────────────
    let orbit_pan_x = RwSignal::new(0.0_f32);
    let orbit_pan_y = RwSignal::new(0.0_f32);
    let orbit_zoom  = RwSignal::new(1.0_f32);
    // dragging: Some((start_client_x, start_client_y, start_pan_x, start_pan_y))
    let orbit_drag: RwSignal<Option<(f32, f32, f32, f32)>> = RwSignal::new(None);

    // Reset view whenever the loaded detail changes
    #[cfg(not(feature = "ssr"))]
    Effect::new(move |_| {
        let _ = detail.get();
        orbit_pan_x.set(0.0);
        orbit_pan_y.set(0.0);
        orbit_zoom.set(1.0);
        orbit_drag.set(None);
    });

    // ── Event handlers (SSR stubs / WASM implementations) ────────
    #[cfg(feature = "ssr")]
    let on_orbit_wheel       = move |_: leptos::ev::WheelEvent|   {};
    #[cfg(feature = "ssr")]
    let on_orbit_pointerdown = move |_: leptos::ev::PointerEvent| {};
    #[cfg(feature = "ssr")]
    let on_orbit_pointermove = move |_: leptos::ev::PointerEvent| {};
    #[cfg(feature = "ssr")]
    let on_orbit_pointerup   = move |_: leptos::ev::PointerEvent| {};
    #[cfg(feature = "ssr")]
    let on_orbit_pointercancel = move |_: leptos::ev::PointerEvent| {};

    #[cfg(not(feature = "ssr"))]
    let on_orbit_wheel = move |e: leptos::ev::WheelEvent| {
        e.prevent_default();
        use wasm_bindgen::JsCast;
        let Some(target) = e.current_target() else { return };
        let el = target.unchecked_ref::<web_sys::Element>();
        let rect = el.get_bounding_client_rect();
        let svg_w = rect.width() as f32;
        let svg_h = rect.height() as f32;
        if svg_w == 0.0 { return }
        // Convert client coords → SVG viewBox coords (400 × 200)
        let cx = (e.client_x() as f32 - rect.left() as f32) / svg_w * 400.0;
        let cy = (e.client_y() as f32 - rect.top() as f32)  / svg_h * 200.0;
        let factor = if e.delta_y() < 0.0 { 1.25 } else { 1.0 / 1.25 };
        let old_z  = orbit_zoom.get_untracked();
        let new_z  = (old_z * factor).clamp(0.1, 200.0);
        let old_px = orbit_pan_x.get_untracked();
        let old_py = orbit_pan_y.get_untracked();
        // Zoom centred on cursor: keep the world-point under cursor fixed
        orbit_pan_x.set(cx - (cx - old_px) * new_z / old_z);
        orbit_pan_y.set(cy - (cy - old_py) * new_z / old_z);
        orbit_zoom.set(new_z);
    };

    #[cfg(not(feature = "ssr"))]
    let on_orbit_pointerdown = move |e: leptos::ev::PointerEvent| {
        orbit_drag.set(Some((
            e.client_x() as f32,
            e.client_y() as f32,
            orbit_pan_x.get_untracked(),
            orbit_pan_y.get_untracked(),
        )));
    };

    #[cfg(not(feature = "ssr"))]
    let on_orbit_pointermove = move |e: leptos::ev::PointerEvent| {
        let Some((sx, sy, px0, py0)) = orbit_drag.get_untracked() else { return };
        use wasm_bindgen::JsCast;
        let Some(target) = e.current_target() else { return };
        let el = target.unchecked_ref::<web_sys::Element>();
        let rect = el.get_bounding_client_rect();
        let svg_w = rect.width() as f32;
        let svg_h = rect.height() as f32;
        if svg_w == 0.0 { return }
        let dx = (e.client_x() as f32 - sx) / svg_w * 400.0;
        let dy = (e.client_y() as f32 - sy) / svg_h * 200.0;
        orbit_pan_x.set(px0 + dx);
        orbit_pan_y.set(py0 + dy);
    };

    #[cfg(not(feature = "ssr"))]
    let on_orbit_pointerup = move |_: leptos::ev::PointerEvent| {
        orbit_drag.set(None);
    };

    #[cfg(not(feature = "ssr"))]
    let on_orbit_pointercancel = move |_: leptos::ev::PointerEvent| {
        orbit_drag.set(None);
    };

    view! {
        <div class="mt-4 border border-border rounded-lg p-4 bg-surface text-sm">

            // ── Header ───────────────────────────────────────
            <div class="flex items-start justify-between gap-2 mb-3">
                <div>
                    {move || selected.get().map(|ev| view! {
                        <p class="font-mono text-xs font-semibold">
                            {ev.sat_a.clone()} " ↔ " {ev.sat_b.clone()}
                        </p>
                        <p class="text-xs text-muted mt-0.5">
                            {
                                #[cfg(not(feature = "ssr"))]
                                {
                                    let now_ms = js_sys::Date::now();
                                    let delta_s = ((ev.tca_unix_ms - now_ms) / 1000.0).max(0.0) as u64;
                                    let h = delta_s / 3600;
                                    let m = (delta_s % 3600) / 60;
                                    format!("TCA in {h}h {m:02}m  •  {:.3} km miss  •  {:.2} km/s",
                                        ev.miss_distance_km, ev.rel_velocity_km_s)
                                }
                                #[cfg(feature = "ssr")]
                                { String::new() }
                            }
                        </p>
                    })}
                </div>
                <button
                    class="text-muted hover:text-foreground transition-colors text-lg leading-none shrink-0"
                    on:click=on_close
                >
                    "✕"
                </button>
            </div>

            // ── Loading state ────────────────────────────────
            <Show when=move || loading.get()>
                <p class="text-xs text-muted animate-pulse">"Computing orbits…"</p>
            </Show>

            // ── Detail content ───────────────────────────────
            <Show when=move || detail.get().is_some()>
                {move || detail.get().map(|d| {

                    // Build SVG distance chart
                    let n = d.distance_profile_km.len();
                    let max_d = d.distance_profile_km.iter().cloned().fold(0.0_f32, f32::max).max(1.0);
                    let w = 560.0_f32;
                    let h_svg = 104.0_f32;
                    let pad_l = 36.0_f32;
                    let pad_t = 14.0_f32; // top padding so y-axis labels don't clip
                    let pad_b = 16.0_f32;
                    let chart_w = w - pad_l;
                    let chart_h = h_svg - pad_t - pad_b;
                    let to_x = |i: usize| pad_l + (i as f32 / (n - 1).max(1) as f32) * chart_w;
                    let to_y = |v: f32| pad_t + (1.0 - v / max_d) * chart_h;

                    let points: String = d.distance_profile_km.iter().enumerate()
                        .map(|(i, &v)| format!("{:.1},{:.1}", to_x(i), to_y(v)))
                        .collect::<Vec<_>>()
                        .join(" ");

                    let threshold_y = to_y(10.0_f32.min(max_d));

                    // TCA step index (approximate)
                    let tca_step = d.distance_profile_km.iter().enumerate()
                        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    let tca_x = to_x(tca_step);

                    // Y-axis labels — use enough precision for small distances
                    let fmt_label = |v: f32| -> String {
                        if v >= 10.0 { format!("{:.0}", v) }
                        else if v >= 1.0 { format!("{:.1}", v) }
                        else { format!("{:.2}", v) }
                    };
                    let y_top_label = fmt_label(max_d);
                    let y_mid_label = fmt_label(max_d / 2.0);

                    // ── Orbital-plane projection SVG — TCA-centred zoom ──────────────
                    // Centre on the TCA midpoint and zoom to ~1/6 of orbit radius so the
                    // step-dots and close-approach geometry are actually visible.  Earth
                    // is drawn to scale but will mostly be clipped below the view.
                    let sat_a_name = d.sat_a.name.clone();
                    let sat_b_name = d.sat_b.name.clone();
                    let orbit_svg: Option<String> = if !d.proj_a.is_empty() && !d.proj_b.is_empty() {
                        let orb_w = 400.0_f32;
                        let orb_h = 200.0_f32;

                        // TCA midpoint (km, in orbital-plane coords)
                        let tca_cx = (d.tca_proj_a[0] + d.tca_proj_b[0]) / 2.0;
                        let tca_cy = (d.tca_proj_a[1] + d.tca_proj_b[1]) / 2.0;

                        // Orbit radius (km from Earth centre to TCA)
                        let orbit_r = (tca_cx * tca_cx + tca_cy * tca_cy).sqrt();

                        // Zoom range: ~1/5 of orbit radius, at least 300 km so the arc
                        // shows clear curvature; expanded to fit the miss distance + margin
                        let miss_margin = (d.miss_distance_km * 50.0).max(300.0_f32);
                        let zoom_range = (orbit_r / 5.0).max(miss_margin);

                        // px/km scale so the zoom_range fills the shorter SVG axis
                        let scale = (orb_w.min(orb_h) - 24.0) / (2.0 * zoom_range);

                        let mx = |x: f32| orb_w / 2.0 + (x - tca_cx) * scale;
                        let my = |y: f32| orb_h / 2.0 - (y - tca_cy) * scale;

                        // Earth — to scale, positioned correctly (mostly off-screen below)
                        let earth_r_px = 6371.0_f32 * scale;
                        let ecx = mx(0.0);
                        let ecy = my(0.0);
                        let mut s = format!(
                            "<style>\
                             .sl .hit{{fill:none;stroke:transparent;stroke-width:6;pointer-events:stroke}}\
                             .sl .vis{{stroke:#6b7280;stroke-width:0.5;opacity:0.4;\
                                       transition:stroke 0.1s,stroke-width 0.1s,opacity 0.1s}}\
                             .sl:hover .vis{{stroke:#e2e8f0;stroke-width:2;opacity:1}}\
                             </style>\
                             <circle cx='{ecx:.1}' cy='{ecy:.1}' r='{earth_r_px:.1}' \
                             fill='#0b1e30' stroke='#1d4ed8' stroke-width='1' opacity='0.8'/>"
                        );

                        // Time-matched step lines (A[i] ↔ B[i]) — drawn beneath dots
                        // Each <g class="sl"> has a fat transparent hit area + visible line + tooltip
                        for (pa, pb) in d.proj_a.iter().zip(d.proj_b.iter()) {
                            let dx = pa[0] - pb[0];
                            let dy = pa[1] - pb[1];
                            let dist_km = (dx * dx + dy * dy).sqrt();
                            let dist_label = if dist_km < 10.0 {
                                format!("{:.3} km", dist_km)
                            } else if dist_km < 100.0 {
                                format!("{:.1} km", dist_km)
                            } else {
                                format!("{:.0} km", dist_km)
                            };
                            let (x1, y1, x2, y2) = (mx(pa[0]), my(pa[1]), mx(pb[0]), my(pb[1]));
                            s.push_str(&format!(
                                "<g class='sl'>\
                                 <line class='hit' x1='{x1:.1}' y1='{y1:.1}' x2='{x2:.1}' y2='{y2:.1}'/>\
                                 <line class='vis' x1='{x1:.1}' y1='{y1:.1}' x2='{x2:.1}' y2='{y2:.1}'/>\
                                 <title>{dist_label}</title>\
                                 </g>"
                            ));
                        }

                        // Step dots — all points rendered; user can pan/zoom to explore
                        for p in &d.proj_a {
                            s.push_str(&format!(
                                "<circle cx='{:.1}' cy='{:.1}' r='2' fill='#60a5fa' opacity='0.65'/>",
                                mx(p[0]), my(p[1])
                            ));
                        }
                        for p in &d.proj_b {
                            s.push_str(&format!(
                                "<circle cx='{:.1}' cy='{:.1}' r='2' fill='#fb923c' opacity='0.65'/>",
                                mx(p[0]), my(p[1])
                            ));
                        }

                        // TCA close-approach line + highlighted endpoints
                        let ax = mx(d.tca_proj_a[0]); let ay = my(d.tca_proj_a[1]);
                        let bx = mx(d.tca_proj_b[0]); let by_ = my(d.tca_proj_b[1]);
                        s.push_str(&format!(
                            "<line x1='{ax:.1}' y1='{ay:.1}' x2='{bx:.1}' y2='{by_:.1}' \
                             stroke='#facc15' stroke-width='2' opacity='0.95'/>\
                             <circle cx='{ax:.1}' cy='{ay:.1}' r='5' \
                             fill='#60a5fa' stroke='#facc15' stroke-width='2'/>\
                             <circle cx='{bx:.1}' cy='{by_:.1}' r='5' \
                             fill='#fb923c' stroke='#facc15' stroke-width='2'/>"
                        ));

                        // Scale bar: 10% of zoom_range, rounded to a nice number
                        let bar_km_raw = zoom_range * 0.2;
                        let mag = 10.0_f32.powf(bar_km_raw.log10().floor());
                        let bar_km = (bar_km_raw / mag).round() * mag;
                        let bar_px = bar_km * scale;
                        let bar_x1 = orb_w - 16.0 - bar_px;
                        let bar_y = orb_h - 12.0;
                        s.push_str(&format!(
                            "<line x1='{bar_x1:.1}' y1='{bar_y}' x2='{:.1}' y2='{bar_y}' \
                             stroke='#888' stroke-width='1.5'/>\
                             <text x='{:.1}' y='{:.1}' font-size='8' fill='#888' text-anchor='middle'>\
                             {bar_km:.0} km</text>",
                            bar_x1 + bar_px,
                            bar_x1 + bar_px / 2.0,
                            bar_y - 3.0,
                        ));

                        Some(s)
                    } else {
                        None
                    };

                    // Hoots bands
                    let hoots_color = if d.hoots_overlap { "text-yellow-400" } else { "text-green-400" };
                    let hoots_label = if d.hoots_overlap { "Altitude bands overlap (Hoots pass)" }
                                      else { "Altitude bands do not overlap (Hoots filtered)" };

                    view! {
                        <div class="space-y-4">

                            // ── Distance chart ────────────────
                            <div>
                                <p class="text-xs text-muted mb-1">
                                    "Inter-satellite distance over 24 h (km, capped at 200)"
                                </p>
                                <svg
                                    viewBox=format!("0 0 {w} {h_svg}")
                                    class="w-full rounded border border-border/40 bg-gray"
                                    style=format!("height:{}px", h_svg as u32)
                                >
                                    // Y-axis labels
                                    <text x="2" y={to_y(max_d) + 4.0}
                                          font-size="9" fill="#888">{y_top_label}</text>
                                    <text x="2" y={to_y(max_d / 2.0) + 4.0}
                                          font-size="9" fill="#888">{y_mid_label}</text>
                                    <text x="2" y={h_svg - pad_b + 1.0}
                                          font-size="9" fill="#888">"0"</text>

                                    // 10 km threshold line
                                    <line
                                        x1=pad_l y1=threshold_y x2=w y2=threshold_y
                                        stroke="#ef4444" stroke-dasharray="4 3"
                                        stroke-width="1" opacity="0.7"
                                    />
                                    <text x={w - 28.0} y={threshold_y - 2.0}
                                          font-size="8" fill="#ef4444">"10 km"</text>

                                    // Distance polyline
                                    <polyline
                                        points=points
                                        fill="none" stroke="#4ade80" stroke-width="1.5"
                                    />

                                    // TCA vertical marker
                                    <line
                                        x1=tca_x y1=pad_t x2=tca_x y2={h_svg - pad_b}
                                        stroke="#facc15" stroke-dasharray="3 3"
                                        stroke-width="1" opacity="0.8"
                                    />
                                    <text x={tca_x + 2.0} y={pad_t + 6.0}
                                          font-size="8" fill="#facc15">"TCA"</text>

                                    // X-axis labels
                                    <text x=pad_l y={h_svg - 2.0}
                                          font-size="8" fill="#666">"0h"</text>
                                    <text x={pad_l + chart_w / 2.0 - 4.0} y={h_svg - 2.0}
                                          font-size="8" fill="#666">"12h"</text>
                                    <text x={w - 14.0} y={h_svg - 2.0}
                                          font-size="8" fill="#666">"24h"</text>
                                </svg>
                            </div>

                            // ── Orbit visualization ───────────
                            {orbit_svg.map(|inner| view! {
                                <div>
                                    <p class="text-xs text-muted mb-1">
                                        "Orbital plane view · 288 steps × 5 min"
                                    </p>
                                    <div class="flex gap-3 text-xs text-muted mb-1">
                                        <span class="flex items-center gap-1">
                                            <span class="inline-block w-2 h-2 rounded-full bg-blue-400"></span>
                                            {sat_a_name.clone()}
                                        </span>
                                        <span class="flex items-center gap-1">
                                            <span class="inline-block w-2 h-2 rounded-full bg-orange-400"></span>
                                            {sat_b_name.clone()}
                                        </span>
                                        <span class="flex items-center gap-1">
                                            <span class="inline-block w-2 h-2 rounded-full bg-yellow-400"></span>
                                            "TCA"
                                        </span>
                                    </div>
                                    <svg
                                        viewBox="0 0 400 200"
                                        class="w-full rounded border border-border/40 bg-gray"
                                        style=move || format!(
                                            "aspect-ratio:2/1;cursor:{}",
                                            if orbit_drag.get().is_some() { "grabbing" } else { "grab" }
                                        )
                                        on:wheel=on_orbit_wheel
                                        on:pointerdown=on_orbit_pointerdown
                                        on:pointermove=on_orbit_pointermove
                                        on:pointerup=on_orbit_pointerup
                                        on:pointercancel=on_orbit_pointercancel
                                    >
                                        <g
                                            transform=move || format!(
                                                "translate({:.2} {:.2}) scale({:.4})",
                                                orbit_pan_x.get(), orbit_pan_y.get(), orbit_zoom.get()
                                            )
                                            inner_html=inner
                                        />
                                    </svg>
                                </div>
                            })}

                            // ── Orbital parameters ────────────
                            <div>
                                <p class="text-xs text-muted mb-1">"Orbital Parameters"</p>
                                <table class="w-full border-collapse">
                                    <thead>
                                        <tr class="border-b border-border">
                                            <th class="py-1 pr-3 text-left text-xs text-muted font-normal w-28">""</th>
                                            <th class="py-1 pr-4 text-left text-xs text-muted font-normal font-mono">
                                                {d.sat_a.name.clone()}
                                            </th>
                                            <th class="py-1 text-left text-xs text-muted font-normal font-mono">
                                                {d.sat_b.name.clone()}
                                            </th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {param_row("Period",
                                            format!("{:.1} min", d.sat_a.period_minutes),
                                            format!("{:.1} min", d.sat_b.period_minutes))}
                                        {param_row("Semi-major axis",
                                            format!("{:.0} km", d.sat_a.semi_major_axis_km),
                                            format!("{:.0} km", d.sat_b.semi_major_axis_km))}
                                        {param_row("Eccentricity",
                                            format!("{:.6}", d.sat_a.eccentricity),
                                            format!("{:.6}", d.sat_b.eccentricity))}
                                        {param_row("Inclination",
                                            format!("{:.2}°", d.sat_a.inclination_deg),
                                            format!("{:.2}°", d.sat_b.inclination_deg))}
                                        {param_row("RAAN",
                                            format!("{:.2}°", d.sat_a.raan_deg),
                                            format!("{:.2}°", d.sat_b.raan_deg))}
                                        {param_row("Arg perigee",
                                            format!("{:.2}°", d.sat_a.arg_perigee_deg),
                                            format!("{:.2}°", d.sat_b.arg_perigee_deg))}
                                        {param_row("Perigee alt",
                                            format!("{:.0} km", d.sat_a.perigee_alt_km),
                                            format!("{:.0} km", d.sat_b.perigee_alt_km))}
                                        {param_row("Apogee alt",
                                            format!("{:.0} km", d.sat_a.apogee_alt_km),
                                            format!("{:.0} km", d.sat_b.apogee_alt_km))}
                                    </tbody>
                                </table>
                            </div>

                            // ── Hoots filter ──────────────────
                            <div class="flex items-center gap-2">
                                <span class=format!("text-xs font-mono {hoots_color}")>
                                    {if d.hoots_overlap { "●" } else { "○" }}
                                </span>
                                <span class="text-xs text-muted">"Hoots filter: "</span>
                                <span class="text-xs">{hoots_label}</span>
                            </div>

                            // ── TLE lines (collapsible) ───────
                            <details class="text-xs">
                                <summary class="cursor-pointer text-muted hover:text-foreground \
                                               transition-colors select-none">
                                    "TLE data"
                                </summary>
                                <div class="mt-2 space-y-2">
                                    <div>
                                        <p class="text-muted mb-0.5">{d.sat_a.name.clone()}</p>
                                        <pre class="font-mono text-[10px] bg-gray rounded p-2 \
                                                   overflow-x-auto border border-border/40">
                                            {format!("{}\n{}", d.tle_a_line1, d.tle_a_line2)}
                                        </pre>
                                    </div>
                                    <div>
                                        <p class="text-muted mb-0.5">{d.sat_b.name.clone()}</p>
                                        <pre class="font-mono text-[10px] bg-gray rounded p-2 \
                                                   overflow-x-auto border border-border/40">
                                            {format!("{}\n{}", d.tle_b_line1, d.tle_b_line2)}
                                        </pre>
                                    </div>
                                </div>
                            </details>

                        </div>
                    }.into_any()
                })}
            </Show>
        </div>
    }
}

