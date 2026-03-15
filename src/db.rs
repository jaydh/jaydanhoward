#[cfg(feature = "ssr")]
pub use inner::*;

#[cfg(feature = "ssr")]
mod inner {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use sqlx::{PgPool, Row};

    pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
        let pool = PgPool::connect(database_url).await?;
        // sqlx acquires a pg_advisory_xact_lock internally to coordinate concurrent runners.
        sqlx::migrate!().run(&pool).await?;
        Ok(pool)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_visit(
        pool: &PgPool,
        ip: &str,
        path: &str,
        country: Option<&str>,
        country_code: Option<&str>,
        region: Option<&str>,
        city: Option<&str>,
        lat: Option<f64>,
        lon: Option<f64>,
        isp: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO visitors (ip, path, country, country_code, region, city, lat, lon, isp)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(ip)
        .bind(path)
        .bind(country)
        .bind(country_code)
        .bind(region)
        .bind(city)
        .bind(lat)
        .bind(lon)
        .bind(isp)
        .execute(pool)
        .await?;

        Ok(())
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CountryStat {
        pub country: String,
        pub country_code: String,
        pub count: i64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RecentVisit {
        pub country: Option<String>,
        pub country_code: Option<String>,
        pub city: Option<String>,
        pub path: String,
        pub minutes_ago: i64,
        pub visited_at: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct VisitorPoint {
        pub lat: f64,
        pub lon: f64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct VisitorStats {
        pub unique_ips: i64,
        pub unique_countries: i64,
        pub total_visits: i64,
        pub top_countries: Vec<CountryStat>,
        pub recent_visits: Vec<RecentVisit>,
        pub points: Vec<VisitorPoint>,
    }

    pub async fn get_visitor_stats(pool: &PgPool) -> Result<VisitorStats, sqlx::Error> {
        let unique_ips: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT ip) FROM visitors WHERE visited_at > NOW() - INTERVAL '30 days'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(Some(0))
        .unwrap_or(0);

        let unique_countries: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT country_code) FROM visitors WHERE visited_at > NOW() - INTERVAL '30 days' AND country_code IS NOT NULL",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(Some(0))
        .unwrap_or(0);

        // Unique visitor-days: a more meaningful "visits" metric than raw request count.
        let total_visits: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM (SELECT DISTINCT ip, visited_at::date FROM visitors WHERE visited_at > NOW() - INTERVAL '30 days') sub",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(Some(0))
        .unwrap_or(0);

        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(country, 'Unknown') as country,
                COALESCE(country_code, 'XX') as country_code,
                COUNT(DISTINCT ip) as count
            FROM visitors
            WHERE visited_at > NOW() - INTERVAL '30 days'
                AND country_code IS NOT NULL
            GROUP BY country, country_code
            ORDER BY count DESC
            LIMIT 10
            "#,
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let top_countries: Vec<CountryStat> = rows
            .into_iter()
            .map(|row| {
                use sqlx::Row;
                CountryStat {
                    country: row.try_get("country").unwrap_or_default(),
                    country_code: row.try_get("country_code").unwrap_or_default(),
                    count: row.try_get("count").unwrap_or(0),
                }
            })
            .collect();

        // One entry per unique IP, most recent visit.
        let recent_rows = sqlx::query(
            r#"
            SELECT country, country_code, city, path, visited_at
            FROM (
                SELECT DISTINCT ON (ip) country, country_code, city, path, visited_at
                FROM visitors
                WHERE visited_at > NOW() - INTERVAL '30 days'
                ORDER BY ip, visited_at DESC
            ) deduped
            ORDER BY visited_at DESC
            LIMIT 20
            "#,
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let now = Utc::now();
        let recent_visits: Vec<RecentVisit> = recent_rows
            .into_iter()
            .map(|row| {
                use sqlx::Row;
                let visited_at: DateTime<Utc> = row.try_get("visited_at").unwrap_or(now);
                let minutes_ago = (now - visited_at).num_minutes().max(0);
                RecentVisit {
                    country: row.try_get("country").ok(),
                    country_code: row.try_get("country_code").ok(),
                    city: row.try_get("city").ok(),
                    path: row.try_get("path").unwrap_or_default(),
                    minutes_ago,
                    visited_at: visited_at.format("%Y-%m-%d %H:%M UTC").to_string(),
                }
            })
            .collect();

        let point_rows = sqlx::query(
            r#"
            SELECT ROUND(lat::numeric, 1)::float8 as lat, ROUND(lon::numeric, 1)::float8 as lon
            FROM visitors
            WHERE visited_at > NOW() - INTERVAL '30 days'
                AND lat IS NOT NULL AND lon IS NOT NULL
            GROUP BY ROUND(lat::numeric, 1), ROUND(lon::numeric, 1)
            LIMIT 500
            "#,
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let points: Vec<VisitorPoint> = point_rows
            .into_iter()
            .map(|row| {
                use sqlx::Row;
                VisitorPoint {
                    lat: row.try_get("lat").unwrap_or(0.0),
                    lon: row.try_get("lon").unwrap_or(0.0),
                }
            })
            .collect();

        Ok(VisitorStats {
            unique_ips,
            unique_countries,
            total_visits,
            top_countries,
            recent_visits,
            points,
        })
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct IpVisit {
        pub path: String,
        pub minutes_ago: i64,
        pub visited_at: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct IpInfo {
        pub ip: String,
        pub country: Option<String>,
        pub country_code: Option<String>,
        pub city: Option<String>,
        pub region: Option<String>,
        pub isp: Option<String>,
        pub history: Vec<IpVisit>,
    }

    pub async fn get_ip_info(pool: &PgPool, ip: &str) -> Result<IpInfo, sqlx::Error> {
        use sqlx::Row;

        let meta_row = sqlx::query(
            r#"
            SELECT country, country_code, city, region, isp
            FROM visitors
            WHERE ip = $1
            ORDER BY visited_at DESC
            LIMIT 1
            "#,
        )
        .bind(ip)
        .fetch_optional(pool)
        .await?;

        let (country, country_code, city, region, isp) = match meta_row {
            Some(row) => (
                row.try_get("country").ok().flatten(),
                row.try_get("country_code").ok().flatten(),
                row.try_get("city").ok().flatten(),
                row.try_get("region").ok().flatten(),
                row.try_get("isp").ok().flatten(),
            ),
            None => (None, None, None, None, None),
        };

        let now = Utc::now();
        let history_rows = sqlx::query(
            r#"
            SELECT path, visited_at
            FROM visitors
            WHERE ip = $1
            ORDER BY visited_at DESC
            LIMIT 20
            "#,
        )
        .bind(ip)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let history = history_rows
            .into_iter()
            .map(|row| {
                let visited_at: DateTime<Utc> = row.try_get("visited_at").unwrap_or(now);
                IpVisit {
                    path: row.try_get("path").unwrap_or_default(),
                    minutes_ago: (now - visited_at).num_minutes().max(0),
                    visited_at: visited_at.format("%Y-%m-%d %H:%M UTC").to_string(),
                }
            })
            .collect();

        Ok(IpInfo { ip: ip.to_string(), country, country_code, city, region, isp, history })
    }

    pub async fn delete_ip_visits(pool: &PgPool, ip: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM visitors WHERE ip = $1")
            .bind(ip)
            .execute(pool)
            .await?;
        Ok(())
    }

    // ──────────────────────────────────────────────────────────────
    // Conjunction screening DB functions
    // ──────────────────────────────────────────────────────────────

    // ── Chunk work-queue ─────────────────────────────────────────

    /// Metadata returned when a pod claims a chunk.
    #[derive(Debug)]
    pub struct ChunkInfo {
        pub chunk_id:    i64,
        pub screening_id: i64,
        pub group_name:  String,
        #[allow(dead_code)]
        pub chunk_idx:   i32,
        pub sat_start:   usize,
        pub sat_end:     usize,
    }

    /// Insert one `pending` chunk row per satellite range for a given screening.
    /// `chunk_size` satellites per chunk; last chunk may be smaller.
    pub async fn create_chunks(
        pool: &PgPool,
        screening_id: i64,
        group_name: &str,
        n_sats: usize,
        chunk_size: usize,
    ) -> Result<usize, sqlx::Error> {
        if n_sats == 0 { return Ok(0); }
        let chunk_size = chunk_size.max(1);
        let n_chunks = n_sats.div_ceil(chunk_size);

        let mut q = String::from(
            "INSERT INTO conjunction_chunks \
             (screening_id, group_name, chunk_idx, sat_start, sat_end) VALUES ",
        );
        let mut placeholders = Vec::with_capacity(n_chunks);
        for i in 0..n_chunks {
            let base = i * 5;
            placeholders.push(format!(
                "(${}, ${}, ${}, ${}, ${})",
                base + 1, base + 2, base + 3, base + 4, base + 5
            ));
        }
        q.push_str(&placeholders.join(", "));

        let mut stmt = sqlx::query(&q);
        for i in 0..n_chunks {
            let sat_start = (i * chunk_size) as i32;
            let sat_end   = (((i + 1) * chunk_size).min(n_sats)) as i32;
            stmt = stmt
                .bind(screening_id)
                .bind(group_name)
                .bind(i as i32)
                .bind(sat_start)
                .bind(sat_end);
        }
        stmt.execute(pool).await?;
        Ok(n_chunks)
    }

    /// Atomically claim the next available chunk across all running screenings.
    /// Uses `FOR UPDATE SKIP LOCKED` so concurrent pods never pick the same chunk.
    pub async fn claim_next_chunk(
        pool: &PgPool,
        hostname: &str,
    ) -> Result<Option<ChunkInfo>, sqlx::Error> {
        use sqlx::Row;
        let row = sqlx::query(
            r#"UPDATE conjunction_chunks
               SET status = 'running', claimed_by = $1, claimed_at = NOW()
               WHERE id = (
                   SELECT c.id
                   FROM conjunction_chunks c
                   JOIN conjunction_screenings s ON s.id = c.screening_id
                   WHERE c.status = 'pending'
                     AND s.status = 'running'
                   ORDER BY c.screening_id ASC, c.chunk_idx ASC
                   FOR UPDATE OF c SKIP LOCKED
                   LIMIT 1
               )
               RETURNING id, screening_id, group_name, chunk_idx, sat_start, sat_end"#,
        )
        .bind(hostname)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| ChunkInfo {
            chunk_id:     r.try_get("id").unwrap_or(0),
            screening_id: r.try_get("screening_id").unwrap_or(0),
            group_name:   r.try_get("group_name").unwrap_or_default(),
            chunk_idx:    r.try_get("chunk_idx").unwrap_or(0),
            sat_start:    r.try_get::<i32, _>("sat_start").unwrap_or(0) as usize,
            sat_end:      r.try_get::<i32, _>("sat_end").unwrap_or(0) as usize,
        }))
    }

    /// Mark a chunk complete and roll up progress.
    /// If this was the last pending/running chunk, marks the parent screening complete too.
    /// Returns `true` when the parent screening was just completed.
    pub async fn complete_chunk(
        pool: &PgPool,
        chunk_id: i64,
        screening_id: i64,
        pairs_screened: i64,
        events_found: i32,
        elapsed_ms: i64,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query(
            "UPDATE conjunction_chunks \
             SET status = 'complete', completed_at = NOW(), \
                 pairs_screened = $2, events_found = $3, elapsed_ms = $4 \
             WHERE id = $1",
        )
        .bind(chunk_id)
        .bind(pairs_screened)
        .bind(events_found)
        .bind(elapsed_ms)
        .execute(pool)
        .await?;

        // Recompute pairs_after_hoots on the parent as the sum across all chunks.
        sqlx::query(
            "UPDATE conjunction_screenings \
             SET pairs_after_hoots = (\
                 SELECT COALESCE(SUM(pairs_screened), 0) \
                 FROM conjunction_chunks WHERE screening_id = $1\
             ) WHERE id = $1",
        )
        .bind(screening_id)
        .execute(pool)
        .await?;

        // Complete parent if no chunks remain in a non-terminal state.
        let all_done: bool = sqlx::query_scalar(
            "SELECT NOT EXISTS(\
                 SELECT 1 FROM conjunction_chunks \
                 WHERE screening_id = $1 AND status NOT IN ('complete', 'failed')\
             )",
        )
        .bind(screening_id)
        .fetch_one(pool)
        .await?;

        if all_done {
            sqlx::query(
                "UPDATE conjunction_screenings \
                 SET status = 'complete', completed_at = NOW(), \
                     events_found = (SELECT COUNT(*) FROM conjunction_events WHERE screening_id = $1), \
                     elapsed_ms = EXTRACT(EPOCH FROM (NOW() - started_at))::BIGINT * 1000 \
                 WHERE id = $1 AND status = 'running'",
            )
            .bind(screening_id)
            .execute(pool)
            .await?;
        }

        Ok(all_done)
    }

    /// Mark a chunk failed. Completes the parent screening if all chunks are terminal.
    pub async fn fail_chunk(
        pool: &PgPool,
        chunk_id: i64,
        screening_id: i64,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE conjunction_chunks \
             SET status = 'failed', completed_at = NOW(), error_msg = $2 \
             WHERE id = $1",
        )
        .bind(chunk_id)
        .bind(error)
        .execute(pool)
        .await?;

        let all_done: bool = sqlx::query_scalar(
            "SELECT NOT EXISTS(\
                 SELECT 1 FROM conjunction_chunks \
                 WHERE screening_id = $1 AND status NOT IN ('complete', 'failed')\
             )",
        )
        .bind(screening_id)
        .fetch_one(pool)
        .await?;

        if all_done {
            sqlx::query(
                "UPDATE conjunction_screenings \
                 SET status = 'complete', completed_at = NOW(), \
                     events_found = (SELECT COUNT(*) FROM conjunction_events WHERE screening_id = $1), \
                     elapsed_ms = EXTRACT(EPOCH FROM (NOW() - started_at))::BIGINT * 1000 \
                 WHERE id = $1 AND status = 'running'",
            )
            .bind(screening_id)
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    /// Atomically increment pairs_after_hoots for live progress during chunk screening.
    pub async fn increment_conjunction_pairs_propagated(
        pool: &PgPool,
        screening_id: i64,
        increment: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE conjunction_screenings \
             SET pairs_after_hoots = pairs_after_hoots + $2 \
             WHERE id = $1",
        )
        .bind(screening_id)
        .bind(increment)
        .execute(pool)
        .await?;
        Ok(())
    }

    // ── End chunk work-queue ─────────────────────────────────────

    /// Try to claim a screening slot for the given group.
    ///
    /// Returns `Some(id)` if this replica won the race, `None` if another replica is
    /// already running it.
    ///
    /// Before attempting the insert, any `running` row older than `stale_threshold` is
    /// Like `start_conjunction_screening` but also skips groups that were completed
    /// recently (within `recent_minutes`).  Used at startup so slow-starting pods
    /// don't re-run groups that a faster sibling already finished.
    pub async fn try_claim_conjunction_startup(
        pool: &PgPool,
        group_name: &str,
        calculated_by: &str,
        recent_minutes: i64,
    ) -> Result<Option<i64>, sqlx::Error> {
        // Expire stale running rows first (same crash-recovery logic).
        sqlx::query(
            "UPDATE conjunction_screenings \
             SET status = 'failed', completed_at = NOW(), \
                 error_msg = 'timed out (stale lock recovery)' \
             WHERE group_name = $1 \
               AND status = 'running' \
               AND started_at < NOW() - INTERVAL '3 hours'",
        )
        .bind(group_name)
        .execute(pool)
        .await?;

        // Atomically claim only when no other pod is running OR recently completed.
        let id: Option<i64> = sqlx::query_scalar(
            "INSERT INTO conjunction_screenings (group_name, status, calculated_by, total_pairs) \
             SELECT $1, 'running', $2, 0 \
             WHERE NOT EXISTS ( \
                 SELECT 1 FROM conjunction_screenings \
                 WHERE group_name = $1 \
                   AND ( \
                       status = 'running' \
                       OR (status = 'complete' AND completed_at > NOW() - ($3 || ' minutes')::INTERVAL) \
                   ) \
             ) \
             RETURNING id",
        )
        .bind(group_name)
        .bind(calculated_by)
        .bind(recent_minutes)
        .fetch_optional(pool)
        .await?
        .flatten();

        Ok(id)
    }

    /// marked `failed` (replica crash recovery) so it no longer blocks the unique index.
    pub async fn start_conjunction_screening(
        pool: &PgPool,
        group_name: &str,
        calculated_by: &str,
        total_pairs: i64,
    ) -> Result<Option<i64>, sqlx::Error> {
        // Expire stale locks: a job running for more than 3 hours is assumed dead.
        sqlx::query(
            "UPDATE conjunction_screenings \
             SET status = 'failed', completed_at = NOW(), \
                 error_msg = 'timed out (stale lock recovery)' \
             WHERE group_name = $1 \
               AND status = 'running' \
               AND started_at < NOW() - INTERVAL '3 hours'",
        )
        .bind(group_name)
        .execute(pool)
        .await?;

        // Atomically claim the slot. ON CONFLICT DO NOTHING means only one replica wins.
        let id: Option<i64> = sqlx::query_scalar(
            "INSERT INTO conjunction_screenings (group_name, status, calculated_by, total_pairs) \
             VALUES ($1, 'running', $2, $3) \
             ON CONFLICT DO NOTHING \
             RETURNING id",
        )
        .bind(group_name)
        .bind(calculated_by)
        .bind(total_pairs)
        .fetch_optional(pool)
        .await?
        .flatten();

        Ok(id)
    }

    /// Update the live pairs-propagated count on a running screening.
    pub async fn update_conjunction_pairs_propagated(
        pool: &PgPool,
        screening_id: i64,
        pairs_after_hoots: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE conjunction_screenings SET pairs_after_hoots = $2 WHERE id = $1",
        )
        .bind(screening_id)
        .bind(pairs_after_hoots)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Update total_pairs once TLEs have been fetched (called after pre-claim).
    pub async fn update_conjunction_total_pairs(
        pool: &PgPool,
        screening_id: i64,
        total_pairs: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE conjunction_screenings SET total_pairs = $2 WHERE id = $1",
        )
        .bind(screening_id)
        .bind(total_pairs)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Mark a screening as complete and record stats.
    pub async fn complete_conjunction_screening(
        pool: &PgPool,
        screening_id: i64,
        total_pairs: i64,
        pairs_after_hoots: i64,
        events_found: i32,
        elapsed_ms: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE conjunction_screenings
               SET status = 'complete', completed_at = NOW(),
                   total_pairs = $2, pairs_after_hoots = $3,
                   events_found = $4, elapsed_ms = $5
               WHERE id = $1"#,
        )
        .bind(screening_id)
        .bind(total_pairs)
        .bind(pairs_after_hoots)
        .bind(events_found)
        .bind(elapsed_ms)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Force-cancel any running screening for a group (used before manual retrigger).
    pub async fn cancel_running_conjunction_screening(
        pool: &PgPool,
        group_name: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE conjunction_screenings \
             SET status = 'failed', completed_at = NOW(), \
                 error_msg = 'cancelled by user retrigger' \
             WHERE group_name = $1 AND status = 'running'",
        )
        .bind(group_name)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Mark a screening as failed.
    pub async fn fail_conjunction_screening(
        pool: &PgPool,
        screening_id: i64,
        error_msg: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE conjunction_screenings SET status = 'failed', completed_at = NOW(), error_msg = $2 WHERE id = $1",
        )
        .bind(screening_id)
        .bind(error_msg)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Bulk insert conjunction events for a completed screening.
    pub async fn insert_conjunction_events(
        pool: &PgPool,
        screening_id: i64,
        events: &[crate::components::conjunction::ConjunctionEvent],
    ) -> Result<(), sqlx::Error> {
        if events.is_empty() {
            return Ok(());
        }
        // Build a single multi-row insert for efficiency.
        let mut query = String::from(
            "INSERT INTO conjunction_events \
             (screening_id, sat_a, sat_b, tca_unix_ms, miss_distance_km, rel_velocity_km_s, calculated_by) \
             VALUES ",
        );
        let mut params: Vec<String> = Vec::with_capacity(events.len());
        for (i, _) in events.iter().enumerate() {
            let base = i * 7;
            params.push(format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${})",
                base + 1,
                base + 2,
                base + 3,
                base + 4,
                base + 5,
                base + 6,
                base + 7
            ));
        }
        query.push_str(&params.join(", "));

        let mut q = sqlx::query(&query);
        for e in events {
            q = q
                .bind(screening_id)
                .bind(&e.sat_a)
                .bind(&e.sat_b)
                .bind(e.tca_unix_ms)
                .bind(e.miss_distance_km)
                .bind(e.rel_velocity_km_s)
                .bind(&e.calculated_by);
        }
        q.execute(pool).await?;
        Ok(())
    }

    #[derive(Debug, Clone)]
    pub struct ConjunctionScreeningRow {
        #[allow(dead_code)]
        pub id: i64,
        pub status: String,
        pub started_at: chrono::DateTime<Utc>,
        pub total_pairs: i64,
        pub pairs_after_hoots: i64,
        pub events_found: i32,
        pub elapsed_ms: i64,
        pub error_msg: Option<String>,
    }

    /// Fetch the most recent screening record for a group.
    pub async fn get_latest_conjunction_screening(
        pool: &PgPool,
        group_name: &str,
    ) -> Result<Option<ConjunctionScreeningRow>, sqlx::Error> {
        use sqlx::Row;
        let row = sqlx::query(
            r#"SELECT id, status, started_at, total_pairs, pairs_after_hoots,
                      events_found, elapsed_ms, error_msg
               FROM conjunction_screenings
               WHERE group_name = $1
               ORDER BY id DESC
               LIMIT 1"#,
        )
        .bind(group_name)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| ConjunctionScreeningRow {
            id: r.try_get("id").unwrap_or(0),
            status: r.try_get("status").unwrap_or_default(),
            started_at: r.try_get("started_at").unwrap_or_else(|_| Utc::now()),
            total_pairs: r.try_get("total_pairs").unwrap_or(0),
            pairs_after_hoots: r.try_get("pairs_after_hoots").unwrap_or(0),
            events_found: r.try_get("events_found").unwrap_or(0),
            elapsed_ms: r.try_get("elapsed_ms").unwrap_or(0),
            error_msg: r.try_get("error_msg").ok().flatten(),
        }))
    }

    /// Fetch events for the latest completed screening of a group.
    pub async fn get_latest_conjunction_events(
        pool: &PgPool,
        group_name: &str,
    ) -> Result<Vec<crate::components::conjunction::ConjunctionEvent>, sqlx::Error> {
        use sqlx::Row;
        let rows = sqlx::query(
            r#"SELECT ce.sat_a, ce.sat_b, ce.tca_unix_ms, ce.miss_distance_km, ce.rel_velocity_km_s,
                      ce.calculated_by
               FROM conjunction_events ce
               JOIN conjunction_screenings cs ON ce.screening_id = cs.id
               WHERE cs.id = (
                   SELECT id FROM conjunction_screenings
                   WHERE group_name = $1 AND status IN ('complete', 'running')
                   ORDER BY id DESC
                   LIMIT 1
               )
               ORDER BY ce.tca_unix_ms ASC"#,
        )
        .bind(group_name)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| crate::components::conjunction::ConjunctionEvent {
                sat_a: r.try_get("sat_a").unwrap_or_default(),
                sat_b: r.try_get("sat_b").unwrap_or_default(),
                tca_unix_ms: r.try_get("tca_unix_ms").unwrap_or(0.0),
                miss_distance_km: r.try_get("miss_distance_km").unwrap_or(0.0),
                rel_velocity_km_s: r.try_get("rel_velocity_km_s").unwrap_or(0.0),
                calculated_by: r.try_get("calculated_by").unwrap_or_default(),
            })
            .collect())
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct NetworkInsightRow {
        pub id: i64,
        pub occurred_at: DateTime<Utc>,
        pub spike_tx_mbps: f64,
        pub baseline_tx_mbps: f64,
        pub top_pods: serde_json::Value,
        pub explanation: String,
    }

    pub async fn insert_network_insight(
        pool: &PgPool,
        spike_tx_mbps: f64,
        baseline_tx_mbps: f64,
        top_pods: &serde_json::Value,
        explanation: &str,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query(
            "INSERT INTO network_insights (spike_tx_mbps, baseline_tx_mbps, top_pods, explanation)
             VALUES ($1, $2, $3, $4)
             RETURNING id",
        )
        .bind(spike_tx_mbps)
        .bind(baseline_tx_mbps)
        .bind(top_pods)
        .bind(explanation)
        .fetch_one(pool)
        .await
        .and_then(|r| r.try_get(0))
    }

    pub async fn get_recent_network_insights(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<NetworkInsightRow>, sqlx::Error> {
        sqlx::query(
            "SELECT id, occurred_at, spike_tx_mbps, baseline_tx_mbps, top_pods, explanation
             FROM network_insights
             ORDER BY occurred_at DESC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await
        .map(|rows| rows.into_iter().map(|r| NetworkInsightRow {
            id: r.try_get("id").unwrap_or(0),
            occurred_at: r.try_get("occurred_at").unwrap_or_else(|_| Utc::now()),
            spike_tx_mbps: r.try_get("spike_tx_mbps").unwrap_or(0.0),
            baseline_tx_mbps: r.try_get("baseline_tx_mbps").unwrap_or(0.0),
            top_pods: r.try_get("top_pods").unwrap_or(serde_json::Value::Array(vec![])),
            explanation: r.try_get("explanation").unwrap_or_default(),
        }).collect())
    }

    /// Try to atomically claim the right to explain a spike in the current
    /// 5-minute bucket. Returns true if this pod won the race, false if another
    /// pod already claimed it (or on DB error).
    pub async fn try_claim_spike(pool: &PgPool) -> bool {
        let result = sqlx::query(
            "INSERT INTO spike_claims (bucket) \
             VALUES (date_trunc('5 minutes', NOW())) \
             ON CONFLICT DO NOTHING",
        )
        .execute(pool)
        .await;

        matches!(result, Ok(r) if r.rows_affected() == 1)
    }

    /// Load the persisted spike detector thresholds, falling back to defaults.
    pub async fn load_spike_config(pool: &PgPool) -> (f64, f64) {
        let row = sqlx::query(
            "SELECT multiplier, floor_mbps FROM spike_detector_config WHERE id = 1",
        )
        .fetch_optional(pool)
        .await;

        match row {
            Ok(Some(r)) => (
                r.try_get("multiplier").unwrap_or(3.0),
                r.try_get("floor_mbps").unwrap_or(5.0),
            ),
            _ => (3.0, 5.0),
        }
    }

    /// Persist updated spike detector thresholds.
    pub async fn save_spike_config(
        pool: &PgPool,
        multiplier: f64,
        floor_mbps: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO spike_detector_config (id, multiplier, floor_mbps, updated_at) \
             VALUES (1, $1, $2, NOW()) \
             ON CONFLICT (id) DO UPDATE SET multiplier = $1, floor_mbps = $2, updated_at = NOW()",
        )
        .bind(multiplier)
        .bind(floor_mbps)
        .execute(pool)
        .await?;
        Ok(())
    }
}
