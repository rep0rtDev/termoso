-- Team-wide session recording policy and retention of server-side recordings.
ALTER TABLE team_policies
    ADD COLUMN session_recording text NOT NULL DEFAULT 'optional'
        CHECK (session_recording IN ('optional', 'required')),
    ADD COLUMN log_retention_days int NOT NULL DEFAULT 0
        CHECK (log_retention_days >= 0 AND log_retention_days <= 3650);

CREATE INDEX session_logs_vault_created_idx ON session_logs (vault_id, created_at) WHERE deleted = false;
