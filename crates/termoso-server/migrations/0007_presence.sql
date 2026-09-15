-- Team presence: who is connected to which team-vault host right now.
-- The live state itself is ephemeral (Redis); only the switches persist.
ALTER TABLE teams
    ADD COLUMN presence_enabled boolean NOT NULL DEFAULT false;

ALTER TABLE users
    ADD COLUMN presence_hidden boolean NOT NULL DEFAULT false;
