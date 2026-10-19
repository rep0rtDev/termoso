-- Just-in-time access: members ask for temporary access to a team vault,
-- approvers grant it for N hours, the server revokes it when it expires.
ALTER TABLE vault_members ADD COLUMN expires_at timestamptz;
CREATE INDEX vault_members_expires_idx ON vault_members (expires_at) WHERE expires_at IS NOT NULL;

CREATE TABLE access_requests (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id        uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    vault_id       uuid NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    user_id        uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role           text NOT NULL CHECK (role IN ('viewer', 'editor')),
    hours          int  NOT NULL CHECK (hours >= 1 AND hours <= 720),
    reason         text NOT NULL DEFAULT '',
    status         text NOT NULL DEFAULT 'pending'
                   CHECK (status IN ('pending', 'approved', 'denied', 'cancelled', 'revoked', 'expired')),
    decided_by     uuid REFERENCES users(id) ON DELETE SET NULL,
    decided_at     timestamptz,
    decision_note  text,
    expires_at     timestamptz,
    created_at     timestamptz NOT NULL DEFAULT now(),
    updated_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX access_requests_team_idx ON access_requests (team_id, created_at DESC);
CREATE INDEX access_requests_user_idx ON access_requests (user_id, created_at DESC);
CREATE UNIQUE INDEX access_requests_open_idx ON access_requests (vault_id, user_id) WHERE status = 'pending';
