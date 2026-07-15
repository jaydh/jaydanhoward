-- Ensures at most one 'running' screening per group across all replicas.
-- INSERT ... ON CONFLICT DO NOTHING against this index is the distributed lock.
CREATE UNIQUE INDEX IF NOT EXISTS idx_conjunction_screenings_one_running
    ON conjunction_screenings (group_name)
    WHERE status = 'running';
