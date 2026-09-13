-- Multiplayer: shared live terminal sessions. Metadata only — the terminal
-- stream is end-to-end encrypted and relayed, never stored.
CREATE TABLE live_sessions (
    id              uuid PRIMARY KEY,
    host_user_id    uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    join_token_hash text NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    expires_at      timestamptz NOT NULL,
    ended_at        timestamptz
);
CREATE INDEX live_sessions_host_idx ON live_sessions (host_user_id) WHERE ended_at IS NULL;
