CREATE TABLE tle_cache (
    group_name  TEXT        PRIMARY KEY,
    satellites  JSONB       NOT NULL,
    fetched_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
