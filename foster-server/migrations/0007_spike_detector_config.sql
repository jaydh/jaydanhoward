CREATE TABLE spike_detector_config (
    id          INTEGER PRIMARY KEY DEFAULT 1,
    multiplier  DOUBLE PRECISION NOT NULL DEFAULT 3.0,
    floor_mbps  DOUBLE PRECISION NOT NULL DEFAULT 5.0,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT single_row CHECK (id = 1)
);

INSERT INTO spike_detector_config (id, multiplier, floor_mbps)
VALUES (1, 3.0, 5.0)
ON CONFLICT DO NOTHING;
