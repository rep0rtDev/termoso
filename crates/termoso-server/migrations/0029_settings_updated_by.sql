-- Who last changed the server settings (shown in the admin console).
ALTER TABLE server_settings
    ADD COLUMN IF NOT EXISTS updated_by uuid REFERENCES users(id) ON DELETE SET NULL;
