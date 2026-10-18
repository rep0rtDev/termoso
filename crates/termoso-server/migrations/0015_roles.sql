-- Fine-grained team roles: a role is a named set of permissions; every member
-- holds exactly one role. Built-in roles are seeded per team and cannot be
-- deleted. `team_members.role` (owner|admin|member) stays as the coarse level
-- used by older clients; `role_id` is the source of truth from 0.9.0 on.

CREATE TABLE team_roles (
    id          uuid PRIMARY KEY,
    team_id     uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    slug        text NOT NULL,
    name        text NOT NULL,
    description text NOT NULL DEFAULT '',
    permissions text[] NOT NULL DEFAULT '{}',
    builtin     boolean NOT NULL DEFAULT false,
    position    int NOT NULL DEFAULT 100,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (team_id, slug)
);
CREATE INDEX team_roles_team_idx ON team_roles (team_id, position);

ALTER TABLE team_members
    ADD COLUMN role_id     uuid REFERENCES team_roles(id) ON DELETE RESTRICT,
    ADD COLUMN role_source text NOT NULL DEFAULT 'manual';   -- manual | idp | scim

ALTER TABLE teams
    ADD COLUMN default_role_id uuid REFERENCES team_roles(id) ON DELETE SET NULL;

-- Group (IdP / SCIM) → role mapping; lowest priority number wins.
CREATE TABLE team_role_mappings (
    team_id     uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    group_name  text NOT NULL,
    role_id     uuid NOT NULL REFERENCES team_roles(id) ON DELETE CASCADE,
    priority    int NOT NULL DEFAULT 100,
    created_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (team_id, group_name)
);

-- Seed built-in roles for existing teams and attach members.
INSERT INTO team_roles (id, team_id, slug, name, description, permissions, builtin, position)
SELECT gen_random_uuid(), t.id, r.slug, r.name, r.description, r.permissions, true, r.position
FROM teams t
CROSS JOIN (VALUES
  ('owner', 'Owner', 'Everything, including deleting the team and transferring ownership.',
     ARRAY[
       '*'
     ], 0),
  ('admin', 'Admin', 'Manages members, roles, vaults, policies, audit and integrations.',
     ARRAY[
       'team.view','team.rename','team.settings','members.view','members.view_mfa','members.invite',
       'invites.view','members.remove','members.suspend','members.role','roles.manage',
       'vaults.create','vaults.rename','vaults.archive','vaults.delete','vaults.transfer',
       'vaults.members','vaults.rotate_key','vaults.history','entities.read','entities.write',
       'entities.delete','entities.move','entities.share_link','secrets.reveal','secrets.copy',
       'keys.generate','keys.export','snippets.run','connect.ssh','connect.sftp',
       'connect.port_forward','connect.telnet','connect.serial','connect.mosh',
       'connect.local_terminal','connect.record','connect.bridge','live.start','live.join',
       'live.control','logs.view_own','logs.view_team','logs.export','logs.delete','export.vault',
       'import.vault','bridges.manage','bridges.use','sshid.manage','ai.use','mcp.use',
       'digest.manage','audit.view','audit.view_ips','audit.export','webhooks.manage',
       'tokens.manage','policies.manage','sso.manage','scim.manage','scim.tokens'
     ], 10),
  ('member', 'Member', 'Works with the vaults they are given access to.',
     ARRAY[
       'team.view','members.view','invites.view','entities.read','entities.write','entities.delete',
       'entities.move','entities.share_link','secrets.reveal','secrets.copy','keys.generate',
       'snippets.run','connect.ssh','connect.sftp','connect.port_forward','connect.telnet',
       'connect.serial','connect.mosh','connect.local_terminal','connect.record','connect.bridge',
       'live.start','live.join','live.control','logs.view_own','import.vault','bridges.use',
       'ai.use','mcp.use','audit.view','vaults.history'
     ], 20),
  ('viewer', 'Viewer', 'Connects to hosts but cannot change anything or see secrets.',
     ARRAY[
       'team.view','members.view','entities.read','secrets.copy','connect.ssh','connect.sftp',
       'connect.port_forward','connect.telnet','connect.mosh','connect.local_terminal',
       'connect.bridge','bridges.use','live.join','logs.view_own'
     ], 30),
  ('auditor', 'Auditor', 'Reads the audit log and team session logs; no vault access.',
     ARRAY[
       'team.view','members.view','members.view_mfa','invites.view','audit.view','audit.view_ips',
       'audit.export','logs.view_team'
     ], 40)
) AS r(slug, name, description, permissions, position);

UPDATE team_members m SET role_id = r.id
FROM team_roles r WHERE r.team_id = m.team_id AND r.slug = m.role;

UPDATE teams t SET default_role_id = r.id
FROM team_roles r WHERE r.team_id = t.id AND r.slug = 'member';

ALTER TABLE team_members ALTER COLUMN role_id SET NOT NULL;

-- Invitations may name a fine-grained role; NULL falls back to the coarse
-- `role` level (built-in slug) and then to the team default.
ALTER TABLE team_invites
    ADD COLUMN role_id uuid REFERENCES team_roles(id) ON DELETE SET NULL;
