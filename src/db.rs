#[cfg(feature = "ssr")]
pub use inner::*;

#[cfg(feature = "ssr")]
mod inner {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use sqlx::PgPool;

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

    /// Try to claim a screening slot for the given group.
    ///
    /// Returns `Some(id)` if this replica won the race, `None` if another replica is
    /// already running it.
    ///
    /// Before attempting the insert, any `running` row older than `stale_threshold` is
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

    /// Query the size of the jaydanhoward database in bytes.
    pub async fn get_db_size(pool: &PgPool) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar("SELECT pg_database_size('jaydanhoward')")
            .fetch_one(pool)
            .await
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
            "INSERT INTO conjunction_events (screening_id, sat_a, sat_b, tca_unix_ms, miss_distance_km, rel_velocity_km_s) VALUES ",
        );
        let mut params: Vec<String> = Vec::with_capacity(events.len());
        for (i, _) in events.iter().enumerate() {
            let base = i * 6;
            params.push(format!(
                "(${}, ${}, ${}, ${}, ${}, ${})",
                base + 1,
                base + 2,
                base + 3,
                base + 4,
                base + 5,
                base + 6
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
                .bind(e.rel_velocity_km_s);
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
                      cs.calculated_by
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
}
