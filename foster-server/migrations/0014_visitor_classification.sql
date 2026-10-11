-- Request metadata for telling people from bots. `is_bot` is NULL on rows
-- logged before this migration (unclassified; excluded from both the human
-- and bot stats, and aged out by the 90-day retention task).
ALTER TABLE visitors
    ADD COLUMN user_agent TEXT,
    ADD COLUMN referer TEXT,
    ADD COLUMN status SMALLINT,
    ADD COLUMN is_bot BOOLEAN;

-- One ip-api.com lookup per IP, reused across requests so scanner bursts
-- don't exhaust the free tier (45 req/min). `hosting` NULL = not yet known
-- (seeded rows below), which the logger refreshes when it has budget.
CREATE TABLE ip_geo (
    ip           TEXT PRIMARY KEY,
    country      TEXT,
    country_code TEXT,
    region       TEXT,
    city         TEXT,
    lat          DOUBLE PRECISION,
    lon          DOUBLE PRECISION,
    isp          TEXT,
    hosting      BOOLEAN,
    looked_up_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO ip_geo (ip, country, country_code, region, city, lat, lon, isp, looked_up_at)
SELECT DISTINCT ON (ip) ip, country, country_code, region, city, lat, lon, isp, visited_at
FROM visitors
WHERE isp IS NOT NULL
ORDER BY ip, visited_at DESC;
