-- Termoso initial schema.
-- All user content (hosts, keys, snippets, logs…) is stored as opaque
-- client-encrypted blobs; the server only keeps routing metadata.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ───────────────────────── operator state ─────────────────────────

CREATE TABLE server_secrets (
    name        text PRIMARY KEY,
    value       bytea NOT NULL,           -- encrypted with TERMOSO_MASTER_KEY
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE server_settings (
    id          int PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    data        jsonb NOT NULL,
    updated_at  timestamptz NOT NULL DEFAULT now()
);

-- ───────────────────────── users & auth ─────────────────────────

CREATE TABLE users (
    id                              uuid PRIMARY KEY,
    email                           text NOT NULL,
    email_verified                  boolean NOT NULL DEFAULT false,
    display_name                    text,
    -- OPAQUE registration record (server never sees the password)
    opaque_record                   bytea,
    -- zero-knowledge key hierarchy (all client-encrypted)
    public_key                      text NOT NULL,
    wrapped_private_key             text NOT NULL,
    recovery_wrapped_private_key    text NOT NULL,
    recovery_verifier_hash          text NOT NULL,
    key_version                     int NOT NULL DEFAULT 1,
    -- 2FA
    totp_secret                     bytea,           -- encrypted with master key
    totp_enabled                    boolean NOT NULL DEFAULT false,
    -- flags
    is_admin                        boolean NOT NULL DEFAULT false,
    disabled                        boolean NOT NULL DEFAULT false,
    -- per-user counters
    history_seq                     bigint NOT NULL DEFAULT 0,
    logs_seq                        bigint NOT NULL DEFAULT 0,
    created_at                      timestamptz NOT NULL DEFAULT now(),
    updated_at                      timestamptz NOT NULL DEFAULT now(),
    last_seen_at                    timestamptz
);
CREATE UNIQUE INDEX users_email_lower_idx ON users (lower(email));

CREATE TABLE backup_codes (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash   text NOT NULL,
    used_at     timestamptz
);
CREATE INDEX backup_codes_user_idx ON backup_codes (user_id);

CREATE TABLE webauthn_credentials (
    id              uuid PRIMARY KEY,
    user_id         uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            text NOT NULL,
    credential      jsonb NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    last_used_at    timestamptz
);
CREATE INDEX webauthn_credentials_user_idx ON webauthn_credentials (user_id);

CREATE TABLE devices (
    id              uuid PRIMARY KEY,
    user_id         uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            text NOT NULL,
    platform        text NOT NULL,
    app_version     text,
    created_at      timestamptz NOT NULL DEFAULT now(),
    last_seen_at    timestamptz NOT NULL DEFAULT now(),
    last_ip         text,
    approved_at     timestamptz
);
CREATE INDEX devices_user_idx ON devices (user_id);

CREATE TABLE sessions (
    id              uuid PRIMARY KEY,
    user_id         uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id       uuid NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    token_hash      text NOT NULL UNIQUE,
    created_at      timestamptz NOT NULL DEFAULT now(),
    expires_at      timestamptz NOT NULL,
    last_used_at    timestamptz NOT NULL DEFAULT now(),
    revoked_at      timestamptz
);
CREATE INDEX sessions_user_idx ON sessions (user_id);
CREATE INDEX sessions_device_idx ON sessions (device_id);

CREATE TABLE sso_identities (
    provider    text NOT NULL,
    subject     text NOT NULL,
    user_id     uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    email       text,
    created_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (provider, subject)
);
CREATE INDEX sso_identities_user_idx ON sso_identities (user_id);

CREATE TABLE user_settings (
    user_id     uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    data        text NOT NULL,           -- client-encrypted
    key_version int NOT NULL DEFAULT 1,
    version     bigint NOT NULL DEFAULT 1,
    updated_at  timestamptz NOT NULL DEFAULT now()
);

-- "Software should report to you, not on you": every security-relevant
-- event is visible to the account owner.
CREATE TABLE security_events (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind        text NOT NULL,
    device_id   uuid,
    ip          text,
    user_agent  text,
    details     jsonb,
    created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX security_events_user_idx ON security_events (user_id, created_at DESC);

-- ───────────────────────── teams ─────────────────────────

CREATE TABLE teams (
    id          uuid PRIMARY KEY,
    name        text NOT NULL,
    owner_id    uuid NOT NULL REFERENCES users(id),
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE team_members (
    team_id     uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id     uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        text NOT NULL,            -- owner | admin | member
    joined_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (team_id, user_id)
);
CREATE INDEX team_members_user_idx ON team_members (user_id);

CREATE TABLE team_invites (
    id          uuid PRIMARY KEY,
    team_id     uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    email       text NOT NULL,
    role        text NOT NULL,
    token_hash  text NOT NULL UNIQUE,
    invited_by  uuid NOT NULL REFERENCES users(id),
    vault_ids   uuid[] NOT NULL DEFAULT '{}',
    created_at  timestamptz NOT NULL DEFAULT now(),
    expires_at  timestamptz NOT NULL,
    accepted_at timestamptz,
    accepted_by uuid REFERENCES users(id)
);
CREATE INDEX team_invites_team_idx ON team_invites (team_id);
CREATE INDEX team_invites_email_idx ON team_invites (lower(email));

-- ───────────────────────── vaults ─────────────────────────

CREATE TABLE vaults (
    id          uuid PRIMARY KEY,
    kind        text NOT NULL,            -- personal | team
    owner_id    uuid REFERENCES users(id) ON DELETE CASCADE,   -- personal
    team_id     uuid REFERENCES teams(id) ON DELETE CASCADE,   -- team
    name        text NOT NULL,
    key_version int NOT NULL DEFAULT 1,
    seq         bigint NOT NULL DEFAULT 0, -- per-vault change counter
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    deleted_at  timestamptz,
    CHECK ((kind = 'personal' AND owner_id IS NOT NULL AND team_id IS NULL)
        OR (kind = 'team' AND team_id IS NOT NULL AND owner_id IS NULL))
);
CREATE UNIQUE INDEX vaults_personal_owner_idx ON vaults (owner_id) WHERE kind = 'personal';
CREATE INDEX vaults_team_idx ON vaults (team_id);

CREATE TABLE vault_members (
    vault_id    uuid NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    user_id     uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        text NOT NULL,            -- owner | editor | viewer
    sealed_key  text,                     -- vault key sealed to user's public key (NULL = pending)
    key_version int NOT NULL DEFAULT 1,
    added_by    uuid REFERENCES users(id),
    added_at    timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (vault_id, user_id)
);
CREATE INDEX vault_members_user_idx ON vault_members (user_id);

-- ───────────────────────── synced content ─────────────────────────

CREATE TABLE entities (
    id                  uuid PRIMARY KEY,
    vault_id            uuid NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    kind                text NOT NULL,
    version             bigint NOT NULL DEFAULT 1,
    seq                 bigint NOT NULL,
    deleted             boolean NOT NULL DEFAULT false,
    key_version         int NOT NULL DEFAULT 1,
    data                text NOT NULL,     -- client-encrypted
    created_at          timestamptz NOT NULL DEFAULT now(),
    updated_at          timestamptz NOT NULL DEFAULT now(),
    updated_by_device   uuid
);
CREATE INDEX entities_vault_seq_idx ON entities (vault_id, seq);
CREATE INDEX entities_vault_kind_idx ON entities (vault_id, kind) WHERE NOT deleted;

CREATE TABLE history_entries (
    id          uuid PRIMARY KEY,
    user_id     uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind        text NOT NULL,
    data        text NOT NULL,             -- client-encrypted
    key_version int NOT NULL DEFAULT 1,
    created_at  timestamptz NOT NULL,
    seq         bigint NOT NULL,
    deleted     boolean NOT NULL DEFAULT false
);
CREATE INDEX history_user_seq_idx ON history_entries (user_id, seq);

CREATE TABLE session_logs (
    id          uuid PRIMARY KEY,
    user_id     uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    vault_id    uuid NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    object_key  text NOT NULL,
    meta        text NOT NULL,             -- client-encrypted
    key_version int NOT NULL DEFAULT 1,
    size_bytes  bigint NOT NULL DEFAULT 0,
    completed   boolean NOT NULL DEFAULT false,
    created_at  timestamptz NOT NULL DEFAULT now(),
    seq         bigint NOT NULL,
    deleted     boolean NOT NULL DEFAULT false
);
CREATE INDEX session_logs_user_seq_idx ON session_logs (user_id, seq);
