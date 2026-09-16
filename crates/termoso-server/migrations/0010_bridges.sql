-- API bridges: headless clients run by the user in their own infrastructure.
-- A bridge owns a key pair; vault keys are sealed to it by the cabinet. Its
-- bearer token is a normal session row flagged with bridge_id so the auth
-- layer can confine it to sync + GET /bridge/me.
CREATE TABLE bridges (
    id              uuid PRIMARY KEY,
    user_id         uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id       uuid NOT NULL UNIQUE REFERENCES devices(id) ON DELETE CASCADE,
    name            text NOT NULL,
    public_key      text NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX bridges_user_idx ON bridges (user_id);

CREATE TABLE bridge_vaults (
    bridge_id       uuid NOT NULL REFERENCES bridges(id) ON DELETE CASCADE,
    vault_id        uuid NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    sealed_key      text,
    key_version     int NOT NULL,
    PRIMARY KEY (bridge_id, vault_id)
);
CREATE INDEX bridge_vaults_vault_idx ON bridge_vaults (vault_id);

ALTER TABLE sessions
    ADD COLUMN bridge_id uuid REFERENCES bridges(id) ON DELETE CASCADE;
