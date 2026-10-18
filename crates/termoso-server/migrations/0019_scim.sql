-- SCIM 2.0 provisioning (0.9.0): per-team bearer tokens, groups, group → vault grants.
CREATE TABLE scim_tokens (
    id           uuid PRIMARY KEY,
    team_id      uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    name         text NOT NULL,
    token_hash   text NOT NULL UNIQUE,
    created_by   uuid REFERENCES users(id) ON DELETE SET NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz,
    last_used_at timestamptz,
    revoked_at   timestamptz
);
CREATE INDEX scim_tokens_team_idx ON scim_tokens (team_id);

CREATE TABLE scim_groups (
    id           uuid PRIMARY KEY,
    team_id      uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    external_id  text,
    display_name text NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (team_id, display_name)
);
CREATE TABLE scim_group_members (
    group_id uuid NOT NULL REFERENCES scim_groups(id) ON DELETE CASCADE,
    user_id  uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (group_id, user_id)
);
CREATE INDEX scim_group_members_user_idx ON scim_group_members (user_id);
CREATE TABLE scim_group_vaults (
    group_id   uuid NOT NULL REFERENCES scim_groups(id) ON DELETE CASCADE,
    vault_id   uuid NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    vault_role text NOT NULL,
    PRIMARY KEY (group_id, vault_id)
);

ALTER TABLE users
    ADD COLUMN scim_external_id text,
    ADD COLUMN scim_active      boolean NOT NULL DEFAULT true,
    ADD COLUMN provisioned_at   timestamptz,
    ADD COLUMN deprovisioned_at timestamptz;

-- Vault memberships granted through a SCIM group are removed with the group.
ALTER TABLE vault_members ADD COLUMN granted_by_group uuid REFERENCES scim_groups(id) ON DELETE SET NULL;
