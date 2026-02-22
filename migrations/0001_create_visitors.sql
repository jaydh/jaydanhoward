CREATE TABLE IF NOT EXISTS visitors (
    id BIGSERIAL PRIMARY KEY,
    ip TEXT NOT NULL,
    country TEXT,
    country_code TEXT,
    region TEXT,
    city TEXT,
    lat DOUBLE PRECISION,
    lon DOUBLE PRECISION,
    isp TEXT,
    path TEXT NOT NULL,
    visited_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS visitors_visited_at_idx ON visitors (visited_at DESC);
CREATE INDEX IF NOT EXISTS visitors_country_code_idx ON visitors (country_code);
