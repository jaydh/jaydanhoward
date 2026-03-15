CREATE TABLE claude_audit_log (
    id            BIGSERIAL PRIMARY KEY,
    occurred_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    context       TEXT NOT NULL,           -- e.g. 'network_spike'
    model         TEXT NOT NULL,
    prompt        TEXT NOT NULL,
    response      TEXT,                    -- NULL if the call failed
    input_tokens  INTEGER,
    output_tokens INTEGER,
    error         TEXT                     -- NULL if succeeded
);

CREATE INDEX claude_audit_log_occurred_at_idx ON claude_audit_log (occurred_at DESC);
