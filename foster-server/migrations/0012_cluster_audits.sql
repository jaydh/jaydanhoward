CREATE TABLE cluster_audits (
    id            BIGSERIAL PRIMARY KEY,
    occurred_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    summary       TEXT NOT NULL,
    significance  SMALLINT NOT NULL,
    findings      JSONB NOT NULL DEFAULT '[]'
);

CREATE INDEX cluster_audits_occurred_at_idx ON cluster_audits (occurred_at DESC);

-- One row per day bucket, same INSERT ON CONFLICT DO NOTHING race pattern as
-- spike_claims — only the replica that wins the race calls Claude.
CREATE TABLE cluster_audit_claims (
    bucket TIMESTAMPTZ PRIMARY KEY
);
