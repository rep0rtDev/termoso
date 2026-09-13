-- SSH ID: a public handle listing the user's device-bound SSH public keys.
-- Only public keys are stored; private halves never leave the devices.
CREATE TABLE ssh_ids (
    user_id     uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    handle      text NOT NULL UNIQUE,
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE ssh_id_keys (
    id          uuid PRIMARY KEY,
    user_id     uuid NOT NULL REFERENCES ssh_ids(user_id) ON DELETE CASCADE,
    -- NULL for FIDO2 keys: they follow the hardware token, not a device.
    device_id   uuid REFERENCES devices(id) ON DELETE CASCADE,
    key_type    text NOT NULL,
    public_key  text NOT NULL,
    label       text NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX ssh_id_keys_user_idx ON ssh_id_keys (user_id);
-- One key per type per device.
CREATE UNIQUE INDEX ssh_id_keys_device_type_idx ON ssh_id_keys (device_id, key_type) WHERE device_id IS NOT NULL;
-- The same public key is never listed twice for one handle.
CREATE UNIQUE INDEX ssh_id_keys_public_idx ON ssh_id_keys (user_id, public_key);
