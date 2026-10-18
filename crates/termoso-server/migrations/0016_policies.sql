-- Team policies (0.9.0): MFA level, local-key prohibition (client-enforced),
-- session TTL, SSO-only domains, mandatory device approval.
CREATE TABLE team_policies (
    team_id          uuid PRIMARY KEY REFERENCES teams(id) ON DELETE CASCADE,
    mfa              text NOT NULL DEFAULT 'off'   CHECK (mfa IN ('off', 'any', 'passkey')),
    local_keys       text NOT NULL DEFAULT 'allow' CHECK (local_keys IN ('allow', 'deny')),
    session_ttl_hours int CHECK (session_ttl_hours IS NULL OR session_ttl_hours BETWEEN 1 AND 2160),
    sso_only         boolean NOT NULL DEFAULT false,
    sso_provider     text,
    sso_domains      text[] NOT NULL DEFAULT '{}',
    device_approval  boolean NOT NULL DEFAULT false,
    updated_by       uuid REFERENCES users(id) ON DELETE SET NULL,
    updated_at       timestamptz NOT NULL DEFAULT now()
);

INSERT INTO team_policies (team_id, mfa)
SELECT id, CASE WHEN require_mfa THEN 'any' ELSE 'off' END FROM teams;
