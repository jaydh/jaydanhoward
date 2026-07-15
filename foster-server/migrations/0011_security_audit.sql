CREATE TABLE IF NOT EXISTS security_audit (
    id          SERIAL PRIMARY KEY,
    report      JSONB       NOT NULL,
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
