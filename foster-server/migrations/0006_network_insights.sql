CREATE TABLE network_insights (
    id              BIGSERIAL PRIMARY KEY,
    occurred_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    spike_tx_mbps   DOUBLE PRECISION NOT NULL,
    baseline_tx_mbps DOUBLE PRECISION NOT NULL,
    top_pods        JSONB NOT NULL DEFAULT '[]',
    explanation     TEXT NOT NULL
);

CREATE INDEX network_insights_occurred_at_idx ON network_insights (occurred_at DESC);
