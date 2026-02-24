CREATE TABLE IF NOT EXISTS conjunction_screenings (
    id              BIGSERIAL PRIMARY KEY,
    group_name      TEXT        NOT NULL,
    status          TEXT        NOT NULL DEFAULT 'running',  -- 'running' | 'complete' | 'failed'
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ,
    total_pairs     BIGINT      NOT NULL DEFAULT 0,
    pairs_after_hoots BIGINT    NOT NULL DEFAULT 0,
    events_found    INT         NOT NULL DEFAULT 0,
    elapsed_ms      BIGINT      NOT NULL DEFAULT 0,
    error_msg       TEXT
);

CREATE TABLE IF NOT EXISTS conjunction_events (
    id                  BIGSERIAL       PRIMARY KEY,
    screening_id        BIGINT          NOT NULL REFERENCES conjunction_screenings(id) ON DELETE CASCADE,
    sat_a               TEXT            NOT NULL,
    sat_b               TEXT            NOT NULL,
    tca_unix_ms         DOUBLE PRECISION NOT NULL,
    miss_distance_km    REAL            NOT NULL,
    rel_velocity_km_s   REAL            NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_conjunction_screenings_group ON conjunction_screenings (group_name, id DESC);
CREATE INDEX IF NOT EXISTS idx_conjunction_events_screening ON conjunction_events (screening_id);
