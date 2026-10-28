-- Appeals handled on the admin page: unblocked or denied.
ALTER TABLE ip_appeals
    ADD COLUMN resolution  TEXT,
    ADD COLUMN resolved_at TIMESTAMPTZ;
