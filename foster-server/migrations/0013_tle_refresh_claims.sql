-- One row per minute bucket, same INSERT ON CONFLICT DO NOTHING race as
-- spike_claims/cluster_audit_claims — only the replica that wins the race
-- does the live CelesTrak refetch when tle_cache goes stale; the other
-- replicas sleep briefly and re-read tle_cache instead of racing CelesTrak
-- independently.
CREATE TABLE tle_refresh_claims (
    bucket TIMESTAMPTZ PRIMARY KEY
);
