-- Accounts created by accepting a team invitation at sign-up belong to that
-- team until they leave it or are removed: the team owner may delete such an
-- account outright. Accounts that existed before joining never get this set.
ALTER TABLE users ADD COLUMN managed_by_team_id uuid REFERENCES teams(id) ON DELETE SET NULL;
CREATE INDEX users_managed_by_team_idx ON users (managed_by_team_id) WHERE managed_by_team_id IS NOT NULL;

-- Deleting an account must not trip over the bookkeeping it left behind:
-- invitations it sent go away with it, "who accepted / who added" become
-- unknown instead of blocking the delete.
ALTER TABLE team_invites
    DROP CONSTRAINT team_invites_invited_by_fkey,
    ADD CONSTRAINT team_invites_invited_by_fkey
        FOREIGN KEY (invited_by) REFERENCES users(id) ON DELETE CASCADE,
    DROP CONSTRAINT team_invites_accepted_by_fkey,
    ADD CONSTRAINT team_invites_accepted_by_fkey
        FOREIGN KEY (accepted_by) REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE vault_members
    DROP CONSTRAINT vault_members_added_by_fkey,
    ADD CONSTRAINT vault_members_added_by_fkey
        FOREIGN KEY (added_by) REFERENCES users(id) ON DELETE SET NULL;
