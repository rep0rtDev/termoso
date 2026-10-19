-- Data residency label and scheduled (GDPR) deletion of teams.
ALTER TABLE teams
    ADD COLUMN region text,
    ADD COLUMN deletion_scheduled_at timestamptz,
    ADD COLUMN deletion_requested_by uuid REFERENCES users(id) ON DELETE SET NULL;
CREATE INDEX teams_deletion_idx ON teams (deletion_scheduled_at) WHERE deletion_scheduled_at IS NOT NULL;
