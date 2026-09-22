-- A team's first vault is its default: it can be renamed but not deleted, so a
-- team never ends up without a vault. At most one default per team.
ALTER TABLE vaults ADD COLUMN is_default boolean NOT NULL DEFAULT false;

CREATE UNIQUE INDEX vaults_team_default_idx ON vaults (team_id)
    WHERE kind = 'team' AND is_default AND deleted_at IS NULL;

-- Existing teams: the oldest surviving vault becomes the default.
UPDATE vaults v SET is_default = true
  FROM (SELECT DISTINCT ON (team_id) id FROM vaults
        WHERE kind = 'team' AND deleted_at IS NULL
        ORDER BY team_id, created_at, id) first
 WHERE v.id = first.id;
