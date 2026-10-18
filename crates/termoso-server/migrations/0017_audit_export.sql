-- Audit export + retention (0.9.0).
ALTER TABLE team_policies
    ADD COLUMN audit_retention_days int NOT NULL DEFAULT 365
        CHECK (audit_retention_days BETWEEN 30 AND 3650);
CREATE INDEX IF NOT EXISTS team_audit_events_action_idx ON team_audit_events (team_id, action, id DESC);
CREATE INDEX IF NOT EXISTS team_audit_events_created_idx ON team_audit_events (team_id, created_at);
