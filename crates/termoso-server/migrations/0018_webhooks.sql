-- Audit webhooks / SIEM delivery (0.9.0).
CREATE TABLE team_webhooks (
    id                uuid PRIMARY KEY,
    team_id           uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    url               text NOT NULL,
    secret_enc        bytea NOT NULL,
    kind              text NOT NULL CHECK (kind IN ('generic', 'splunk_hec', 'elastic', 'datadog')),
    actions           text[],
    enabled           boolean NOT NULL DEFAULT true,
    created_by        uuid REFERENCES users(id) ON DELETE SET NULL,
    created_at        timestamptz NOT NULL DEFAULT now(),
    last_status       int,
    last_error        text,
    last_delivered_at timestamptz,
    failures          int NOT NULL DEFAULT 0
);
CREATE INDEX team_webhooks_team_idx ON team_webhooks (team_id);

-- Outbox: one row per (webhook, event); claimed with SKIP LOCKED by any replica.
CREATE TABLE audit_deliveries (
    id              bigserial PRIMARY KEY,
    webhook_id      uuid NOT NULL REFERENCES team_webhooks(id) ON DELETE CASCADE,
    event_id        bigint NOT NULL REFERENCES team_audit_events(id) ON DELETE CASCADE,
    attempt         int NOT NULL DEFAULT 0,
    next_at         timestamptz NOT NULL DEFAULT now(),
    status          text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'sending', 'ok', 'dead')),
    response_status int,
    response_body   text,
    delivered_at    timestamptz,
    created_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX audit_deliveries_due_idx ON audit_deliveries (status, next_at) WHERE status IN ('pending', 'sending');
CREATE INDEX audit_deliveries_webhook_idx ON audit_deliveries (webhook_id, id DESC);
