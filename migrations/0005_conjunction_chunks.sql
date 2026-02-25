-- Per-event pod tracking (previously joined from screening; with chunks each pod
-- handles a different satellite range so calculated_by belongs on the event).
ALTER TABLE conjunction_events ADD COLUMN IF NOT EXISTS calculated_by TEXT NOT NULL DEFAULT '';

-- Work queue for parallel chunk-based screening within a group.
-- Each chunk covers satellites [sat_start, sat_end) as the "A" satellite in
-- every (A, B) pair (B > A), so no pair is screened twice.
CREATE TABLE IF NOT EXISTS conjunction_chunks (
    id             BIGSERIAL PRIMARY KEY,
    screening_id   BIGINT NOT NULL REFERENCES conjunction_screenings(id) ON DELETE CASCADE,
    group_name     TEXT   NOT NULL,
    chunk_idx      INT    NOT NULL,
    sat_start      INT    NOT NULL,   -- inclusive index into TLE array
    sat_end        INT    NOT NULL,   -- exclusive index into TLE array
    status         TEXT   NOT NULL DEFAULT 'pending',  -- pending/running/complete/failed
    claimed_by     TEXT,
    claimed_at     TIMESTAMPTZ,
    completed_at   TIMESTAMPTZ,
    pairs_screened BIGINT NOT NULL DEFAULT 0,
    events_found   INT    NOT NULL DEFAULT 0,
    elapsed_ms     BIGINT,
    error_msg      TEXT,
    UNIQUE (screening_id, chunk_idx)
);

-- Partial index used by claim_next_chunk (FOR UPDATE SKIP LOCKED).
CREATE INDEX IF NOT EXISTS idx_conjunction_chunks_claimable
    ON conjunction_chunks (screening_id ASC, chunk_idx ASC)
    WHERE status = 'pending';
