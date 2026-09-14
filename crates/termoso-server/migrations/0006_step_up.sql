-- Step-up re-authentication: a session records when it last proved the
-- password (and second factor). Sensitive account mutations require this to
-- be recent, so a leaked bearer token alone cannot take the account over.
ALTER TABLE sessions ADD COLUMN reauth_at timestamptz;

-- "Start over": delayed, cancellable reset for users who lost both the
-- password and the recovery phrase. The old encrypted data is destroyed when
-- the reset completes; the server holds no key that could recover it.
ALTER TABLE users ADD COLUMN reset_scheduled_for timestamptz;
ALTER TABLE users ADD COLUMN reset_finish_hash text;
ALTER TABLE users ADD COLUMN reset_cancel_hash text;
CREATE UNIQUE INDEX users_reset_finish_idx ON users (reset_finish_hash) WHERE reset_finish_hash IS NOT NULL;
CREATE UNIQUE INDEX users_reset_cancel_idx ON users (reset_cancel_hash) WHERE reset_cancel_hash IS NOT NULL;
