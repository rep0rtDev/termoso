-- Team activity log: who did what in a team (members, invites, vaults, access,
-- encrypted entities, multiplayer). Only routing metadata is recorded — entity
-- payloads stay opaque ciphertext, so an entry names kinds and ids, never data.
CREATE TABLE team_audit_events (
    id          bigserial PRIMARY KEY,
    team_id     uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    actor_id    uuid REFERENCES users(id) ON DELETE SET NULL,
    device_id   uuid,
    action      text NOT NULL,
    vault_id    uuid,
    target_user uuid,
    details     jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX team_audit_events_team_idx ON team_audit_events (team_id, id DESC);
