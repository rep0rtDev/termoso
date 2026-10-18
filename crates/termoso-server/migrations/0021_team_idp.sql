-- Per-team identity provider (0.9.1): a team brings its own OIDC/SAML IdP.
-- Members whose email domain is listed in `domains` are routed to it
-- (home-realm discovery); `jit_join` lets a verified sign-in create the
-- account and join the team with the default role (IdP groups → role via
-- team_role_mappings). The client secret is sealed with the server master key.
CREATE TABLE team_idp (
    team_id             uuid PRIMARY KEY REFERENCES teams(id) ON DELETE CASCADE,
    kind                text NOT NULL DEFAULT 'oidc' CHECK (kind IN ('oidc', 'saml')),
    name                text NOT NULL,
    issuer              text,
    client_id           text,
    client_secret_enc   bytea,
    scopes              text,
    saml_metadata       text,
    saml_idp_entity_id  text,
    domains             text[] NOT NULL DEFAULT '{}',
    enabled             boolean NOT NULL DEFAULT true,
    jit_join            boolean NOT NULL DEFAULT true,
    updated_by          uuid REFERENCES users(id) ON DELETE SET NULL,
    updated_at          timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX team_idp_domains_idx ON team_idp USING gin (domains) WHERE enabled;
