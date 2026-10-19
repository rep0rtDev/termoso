-- Access rules on groups and hosts inside team vaults (0.9.1).
-- Entities are client-encrypted, so the server only knows ids: a rule binds
-- a principal (team role, team member or IdP/SCIM group) to an entity id with
-- an access mode. Clients resolve group → host inheritance and enforce
-- `none`/`connect`/`read`; the server additionally refuses writes/deletes on
-- directly-ruled entities below `edit`. No rule = vault-level access as before.
-- Personal and local vaults are never subject to these rules.
CREATE TABLE entity_acl (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id         uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    vault_id        uuid NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    entity_id       uuid NOT NULL,
    entity_kind     text NOT NULL CHECK (entity_kind IN ('group', 'host')),
    principal_kind  text NOT NULL CHECK (principal_kind IN ('role', 'member', 'group')),
    role_id         uuid REFERENCES team_roles(id) ON DELETE CASCADE,
    user_id         uuid REFERENCES users(id) ON DELETE CASCADE,
    group_name      text,
    -- none: hidden · connect: connect without reading secrets · read: see secrets · edit: full
    mode            text NOT NULL CHECK (mode IN ('none', 'connect', 'read', 'edit')),
    created_by      uuid REFERENCES users(id) ON DELETE SET NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now(),
    CHECK (
        (principal_kind = 'role'   AND role_id IS NOT NULL AND user_id IS NULL AND group_name IS NULL) OR
        (principal_kind = 'member' AND user_id IS NOT NULL AND role_id IS NULL AND group_name IS NULL) OR
        (principal_kind = 'group'  AND group_name IS NOT NULL AND role_id IS NULL AND user_id IS NULL)
    )
);
CREATE INDEX entity_acl_vault_idx ON entity_acl (vault_id);
CREATE INDEX entity_acl_team_idx ON entity_acl (team_id);
CREATE UNIQUE INDEX entity_acl_principal_uq ON entity_acl (
    vault_id, entity_id, principal_kind,
    COALESCE(role_id, '00000000-0000-0000-0000-000000000000'::uuid),
    COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::uuid),
    COALESCE(group_name, '')
);
