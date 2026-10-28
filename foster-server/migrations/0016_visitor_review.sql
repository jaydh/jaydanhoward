-- LLM review of visitor sessions (src/visitor_review.rs).

-- Latest verdict per IP. `last_visit_reviewed` is the newest visit the
-- verdict saw, so an IP is reviewed again only once it has new traffic.
CREATE TABLE ip_verdicts (
    ip                  TEXT PRIMARY KEY,
    verdict             TEXT NOT NULL,   -- human | crawler | scanner | abusive | unclear
    confidence          REAL NOT NULL,
    reason              TEXT NOT NULL,
    model               TEXT NOT NULL,
    reviewed_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_visit_reviewed TIMESTAMPTZ NOT NULL
);

-- Active while lifted_at IS NULL AND expires_at > NOW(). Rows are kept after
-- expiry/lifting so repeat offenders get longer blocks and a block lifted by
-- hand (an appeal) isn't immediately re-imposed.
CREATE TABLE ip_blocks (
    ip            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,         -- scanner | abusive
    reason        TEXT NOT NULL,
    blocked_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at    TIMESTAMPTZ NOT NULL,
    lifted_at     TIMESTAMPTZ,
    times_blocked INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE ip_appeals (
    id         BIGSERIAL PRIMARY KEY,
    ip         TEXT NOT NULL,
    message    TEXT NOT NULL,
    contact    TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX ip_appeals_ip_created_idx ON ip_appeals (ip, created_at DESC);

-- One row per review slot; the replica whose INSERT wins runs that review
-- (same pattern as spike_claims / tle_refresh_claims).
CREATE TABLE visitor_review_claims (
    bucket TIMESTAMPTZ PRIMARY KEY
);

-- Shared across replicas, so consecutive failures are counted no matter
-- which replica ran each review.
CREATE TABLE visitor_review_status (
    id                   INTEGER PRIMARY KEY DEFAULT 1,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    last_error           TEXT,
    last_success         TIMESTAMPTZ,
    CONSTRAINT single_row CHECK (id = 1)
);

INSERT INTO visitor_review_status (id) VALUES (1) ON CONFLICT DO NOTHING;
