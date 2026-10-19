-- The daily Claude cluster audit and the Claude audit log were removed
-- (b61b28d); nothing reads or writes these any more.
DROP TABLE IF EXISTS cluster_audit_claims;
DROP TABLE IF EXISTS cluster_audits;
DROP TABLE IF EXISTS claude_audit_log;
