-- Credential rotation: max-age policies, client-reported rotation timestamps
-- and rotation jobs executed by clients that hold the vault key.
ALTER TABLE team_policies
    ADD COLUMN key_max_age_days int NOT NULL DEFAULT 0
        CHECK (key_max_age_days >= 0 AND key_max_age_days <= 3650),
    ADD COLUMN password_max_age_days int NOT NULL DEFAULT 0
        CHECK (password_max_age_days >= 0 AND password_max_age_days <= 3650);

ALTER TABLE entities ADD COLUMN secret_rotated_at timestamptz;

CREATE TABLE rotation_jobs (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id       uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    vault_id      uuid NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    entity_id     uuid NOT NULL,
    entity_kind   text NOT NULL,
    status        text NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending', 'running', 'done', 'failed', 'cancelled')),
    note          text NOT NULL DEFAULT '',
    error         text,
    requested_by  uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    claimed_by    uuid REFERENCES users(id) ON DELETE SET NULL,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now(),
    finished_at   timestamptz
);
CREATE INDEX rotation_jobs_team_idx ON rotation_jobs (team_id, created_at DESC);
CREATE INDEX rotation_jobs_vault_open_idx ON rotation_jobs (vault_id) WHERE status IN ('pending', 'running');
CREATE UNIQUE INDEX rotation_jobs_entity_open_idx ON rotation_jobs (entity_id) WHERE status IN ('pending', 'running');
