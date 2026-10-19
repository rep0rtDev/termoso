# Compliance map

What Termoso implements for the controls that SOC 2 (Trust Services
Criteria), ISO/IEC 27001 Annex A and GDPR auditors ask about, and where each
lives in the code and the product. This is a map, not a certificate: Termoso
the project is not SOC 2 attested; a company running it can use this document
as the system description for its own audit of the self-hosted deployment.

Terminology: *server admin* — operator of the installation (`is_admin`
accounts); *team owner/admin* — roles inside a team; *client* — desktop,
Android, iOS or web cabinet.

## 1. Encryption and key management (CC6.1, CC6.7, A.8.24, GDPR Art. 32)

| Control | Implementation |
| --- | --- |
| Data encrypted at rest, client side | Vault contents (hosts, identities, keys, snippets, session recordings) are encrypted on the client with a per-vault key; the server stores ciphertext only (`crates/termoso-crypto`, `docs/ARCHITECTURE.md`). Backups and the object store inherit this. |
| Key distribution | Vault keys exist on the server only sealed to members' account public keys. Team membership and JIT grants create *pending* memberships; a manager's client seals the key. The server never holds a vault key. |
| Password never reaches the server | OPAQUE password-authenticated key exchange (`crates/termoso-server/src/routes/auth.rs`). |
| Server-side secrets | IdP client secrets, webhook secrets, SCIM tokens are encrypted with the server master key (`TERMOSO_MASTER_KEY`); backups record its fingerprint (`backup.rs`). |
| Transport | TLS terminated by your reverse proxy; HSTS and CSP are set by the server (`routes/mod.rs`). |
| Credential rotation | Team policy `key_max_age_days` / `password_max_age_days`; overdue report and rotation jobs (`routes/rotation.rs`); daily `rotation.due` reminder to webhooks. |

## 2. Identity, authentication, access control (CC6.1–CC6.3, A.5.15–A.5.18, A.8.2–A.8.5)

| Control | Implementation |
| --- | --- |
| SSO | Server-level OIDC/SAML providers and per-team IdPs with home-realm discovery by e-mail domain (`sso.rs`, `routes/idp.rs`). Corporate mode: `sso_only` refuses password sign-in and e-mail sign-up. |
| MFA | TOTP and passkeys (WebAuthn); team policy `mfa = any|passkey` enforced at sign-in and on step-up for sensitive actions. |
| Device trust | New devices need e-mail approval (`device_approval` policy makes it mandatory); device list and revocation per account. |
| Provisioning / deprovisioning | SCIM 2.0 users and groups (`scim.rs`): deactivation removes team and vault access, retention delete after the configured period. |
| Least privilege | Custom team roles with a 60+ permission catalogue (`authz.rs`); vault roles manager/editor/viewer; per-group/per-host ACL inside team vaults (`acl.rs`); "connect without seeing the secret" (`secrets.reveal`). |
| Just-in-time access | Access requests with approval, bounded duration (1–720 h) and automatic revocation (`routes/access.rs`). |
| Session controls | Team policy `session_ttl_hours`; server-wide session limits; sign-out everywhere. |
| Admin separation | Server admins manage the installation (settings, users, teams) but cannot read vault contents or act as a user. |

## 3. Logging, monitoring, audit (CC7.2, CC7.3, A.8.15, A.8.16)

| Control | Implementation |
| --- | --- |
| Team audit log | Every security-relevant team action is recorded with actor, device, target and details (`audit.rs`; ~80 action types). Retention per team (`audit_retention_days`, server cap `TERMOSO_AUDIT_RETENTION_DAYS_MAX`). |
| Export to SIEM | Webhooks (HTTPS JSON, syslog/TLS) with HMAC signatures and delivery log (`webhooks.rs`); CSV/JSONL export; optional JSON-lines sink to stdout (`TERMOSO_AUDIT_SINK`). |
| Session recordings | Server-side storage of encrypted recordings; team policy `session_recording = required` forces recording on all team vaults and restricts deletion to `logs.delete`; `log_retention_days`; team-wide listing and NDJSON export (`routes/logs.rs`). |
| Account security events | Per-user security log (sign-ins, device approvals, MFA changes) in the cabinet. |
| Operational health | `/healthz`, `/readyz`, Prometheus metrics, job locks with last-run timestamps (`jobs.rs`). |

## 4. Data lifecycle, residency, subject rights (GDPR Art. 15, 17, 20, 30, 44; A.5.33, A.8.10)

| Control | Implementation |
| --- | --- |
| Data inventory | See `docs/ARCHITECTURE.md` ("What the server stores"). Personal data held: e-mail, display name, avatar, device metadata, IP in security/audit events; everything else is ciphertext. |
| Access / portability | `GET /teams/{id}/export` — complete team export as one JSON document (members, roles, policies, vaults, entity ciphertext, recording metadata and download links, audit, webhooks, ACL, requests, rotation jobs, IdP settings). Account export for individuals is on the roadmap. |
| Erasure | Account deletion (self-service and admin); team deletion with a cancellable grace period (`TERMOSO_TEAM_DELETION_GRACE_DAYS`, default 7) that purges rows and stored recordings; SCIM deprovisioning with retention delete. |
| Retention | Audit retention per team, recording retention per team, SCIM retention, session limits — all enforced by server jobs. |
| Residency | `TERMOSO_DATA_REGION` labels the installation; it is advertised in `/server/info`, stamped on teams at creation and shown at `GET /teams/{id}/data`. Self-hosting keeps all data in your region by construction. |
| Client currency | `TERMOSO_MIN_CLIENT_VERSION` — oldest client version allowed to sign in (semver). Default: the server's own version — clients older than the server get `client_update_required` and must update. |
| Processing records | Audit log + webhooks give Art. 30 evidence of who did what to team data. |

## 5. Change management and supply chain (CC8.1, A.8.25–A.8.32)

| Control | Implementation |
| --- | --- |
| Source control | All changes through pull requests on GitHub; CI (`ci.yml`) runs format, clippy, tests against PostgreSQL/Redis/S3, web type-check, Android build. |
| Dependency hygiene | `cargo deny` (advisories, licences, sources, bans) and `npm audit` gate every change; Dependabot covers Cargo, npm and the Gradle graph. |
| Reproducible identity of releases | Tags build on GitHub-hosted runners; every asset gets a GitHub build-provenance attestation (`gh attestation verify`); `SHA256SUMS.txt` is signed keylessly with cosign; desktop updates are signed with the Tauri updater key. |
| SBOM | SPDX and CycloneDX SBOMs of the release source tree are published with every release. |
| Vulnerability handling | `SECURITY.md`: private reporting, 3/10-day response targets, 90-day disclosure window. |

## 6. Availability, backup, recovery (A1.2, A1.3, A.8.13, A.8.14)

| Control | Implementation |
| --- | --- |
| HA | Stateless server replicas behind a load balancer, managed PostgreSQL/Redis, S3-compatible object store (`docs/HA.md`). |
| Backup / restore | `termoso-server backup` / `restore` with manifest, schema version and master-key fingerprint checks; weekly restore test in CI (`restore-test.yml`). |
| Jobs | Retention, expiry and reminder jobs take a database lock so only one replica runs them (`jobs.rs`). |

## 7. What is *not* covered

* Termoso has no third-party attestation (SOC 2 report, ISO certificate).
  The hosted cloud at app.termoso.dev is run by the maintainers without one.
* The server is trusted for freshness and membership (see "Threat model" in
  `SECURITY.md`): it cannot read vault data, but it can withhold updates or
  present a wrong public key for a member. Verify member fingerprints
  out-of-band where that matters.
* Physical, HR and vendor-management controls are the operator's.

## Operator checklist for an audit

1. Pin a release and record `gh attestation verify` output and the cosign
   verification of `SHA256SUMS.txt` in your change ticket.
2. Set `TERMOSO_DATA_REGION`, `TERMOSO_AUDIT_RETENTION_DAYS_MAX`,
   `TERMOSO_TEAM_DELETION_GRACE_DAYS` to your policy values.
3. In each team: `mfa = passkey` or `any`, `device_approval = true`,
   `sso_only` with your domains, `session_recording = required` where
   regulated, retention values, `key_max_age_days` / `password_max_age_days`.
4. Point a webhook at your SIEM and keep delivery failures alerting.
5. Schedule `termoso-server backup` and keep a restore drill log.
6. Export the team (`/teams/{id}/export`) once per audit period as evidence
   of scope.
