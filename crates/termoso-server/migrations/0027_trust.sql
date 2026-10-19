-- Network allow-list and admin-approved devices per team.
ALTER TABLE team_policies
    ADD COLUMN ip_allowlist text[] NOT NULL DEFAULT '{}',
    ADD COLUMN device_admin_approval boolean NOT NULL DEFAULT false;

CREATE TABLE team_device_approvals (
    team_id     uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    device_id   uuid NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    user_id     uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status      text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
    decided_by  uuid REFERENCES users(id) ON DELETE SET NULL,
    decided_at  timestamptz,
    created_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (team_id, device_id)
);
CREATE INDEX team_device_approvals_pending_idx ON team_device_approvals (team_id, created_at) WHERE status = 'pending';
