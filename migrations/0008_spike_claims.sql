-- One row per 5-minute bucket. INSERT ON CONFLICT DO NOTHING lets pods race
-- atomically; only the winner (affected rows = 1) calls Claude.
CREATE TABLE spike_claims (
    bucket TIMESTAMPTZ PRIMARY KEY
);
