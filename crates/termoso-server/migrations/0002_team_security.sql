-- Team-level security switches (Settings → Team → Security).
ALTER TABLE teams
    ADD COLUMN multiplayer_enabled boolean NOT NULL DEFAULT true,
    ADD COLUMN require_mfa         boolean NOT NULL DEFAULT false;
