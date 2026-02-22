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
    pub struct IpInfo {
        pub ip: String,
        pub country: Option<String>,
        pub country_code: Option<String>,
        pub city: Option<String>,
        pub region: Option<String>,
        pub isp: Option<String>,
    }

    pub async fn get_ip_info(pool: &PgPool, ip: &str) -> Result<IpInfo, sqlx::Error> {
        let row = sqlx::query(
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

        Ok(match row {
            Some(row) => {
                use sqlx::Row;
                IpInfo {
                    ip: ip.to_string(),
                    country: row.try_get("country").ok().flatten(),
                    country_code: row.try_get("country_code").ok().flatten(),
                    city: row.try_get("city").ok().flatten(),
                    region: row.try_get("region").ok().flatten(),
                    isp: row.try_get("isp").ok().flatten(),
                }
            }
            None => IpInfo {
                ip: ip.to_string(),
                country: None,
                country_code: None,
                city: None,
                region: None,
                isp: None,
            },
        })
    }
}
