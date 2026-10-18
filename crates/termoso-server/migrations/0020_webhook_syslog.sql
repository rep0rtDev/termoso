-- Fifth webhook kind: syslog over TLS (RFC 5425 framing, RFC 5424 header,
-- one CEF line per audit event) for SIEMs that ingest syslog rather than HTTP.
ALTER TABLE team_webhooks DROP CONSTRAINT IF EXISTS team_webhooks_kind_check;
ALTER TABLE team_webhooks
    ADD CONSTRAINT team_webhooks_kind_check
    CHECK (kind IN ('generic', 'splunk_hec', 'elastic', 'datadog', 'syslog_tls'));
