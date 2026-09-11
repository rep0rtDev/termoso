# Termoso

A free, open-source, self-hostable SSH/SFTP workstation in the spirit of Termius:
Rust backend, Tauri desktop clients, React Native mobile clients and a web cabinet.
Every feature is available to every user — there are no tiers, seats or paywalls.

Termoso follows one rule: **software reports to you, not on you**. There is no
telemetry, no analytics, no crash reporting and no "phone home". The only metrics
that exist are opt-in Prometheus counters served on a private listener for the
operator running the server.

> Status: **phase 1 — backend**. The server API is functional and covered by
> integration tests; clients (desktop / mobile / web cabinet) come next.

## Repository layout

```
crates/
  termoso-crypto   client + server cryptography (OPAQUE, Argon2id, XChaCha20-Poly1305,
                   X25519 sealed boxes, BIP39-style recovery key)
  termoso-proto    API request/response types shared by server and clients
  termoso-server   the API server (axum + PostgreSQL + Redis + S3)
deploy/            Dockerfile, docker-compose for production and for local development
```

## Security model (short)

* **Zero-knowledge vaults.** Hosts, keys, snippets, etc. are encrypted on the
  client with per-vault keys. The server stores opaque ciphertext plus the
  minimum metadata needed for sync (ids, versions, timestamps).
* **OPAQUE** password authentication: the server never sees the password and
  cannot brute-force it offline from a stolen database.
* Each account has an **X25519 key pair**. Vault keys are delivered to members as
  sealed boxes for their public key; the private key is wrapped by a KEK derived
  from the OPAQUE export key (Argon2id) and, separately, by a **24-word recovery
  key** so a forgotten password does not mean lost data.
* Server-side secrets (OPAQUE server setup, TOTP secrets) are encrypted at rest
  with `TERMOSO_MASTER_KEY`.
* Sessions are opaque bearer tokens bound to a registered device; devices can be
  listed and revoked from any signed-in client and revocation is pushed live.
* MFA: TOTP, WebAuthn/passkeys, e-mail codes and backup codes. New-device login
  approval by e-mail.
* Session logs (terminal recordings) are encrypted client-side and uploaded via
  pre-signed URLs to S3-compatible storage.

## Running it

### Production (Docker Compose)

```bash
git clone https://github.com/rep0rtDev/termoso.git && cd termoso
cp deploy/.env.example deploy/.env
$EDITOR deploy/.env              # at least TERMOSO_MASTER_KEY, passwords and public URLs
docker compose -f deploy/docker-compose.yml up -d
```

This starts the API (`:8080`), PostgreSQL, Redis and MinIO. Put a TLS-terminating
reverse proxy (Caddy, Traefik, nginx) in front of the API and of MinIO (for
pre-signed log uploads) and set `TERMOSO_PUBLIC_URL`, `TERMOSO_WEB_URL` and
`TERMOSO_S3__PUBLIC_ENDPOINT` accordingly. Migrations run automatically on start.

* `GET /healthz` — liveness, `GET /readyz` — readiness (checks PostgreSQL and Redis).
* `GET /api/openapi.json` — OpenAPI 3.1; `GET /docs` — Swagger UI when
  `TERMOSO_SWAGGER_UI=true`.

The API is stateless: run as many replicas as you like behind one proxy. Shared
state lives in PostgreSQL and Redis (sessions cache, rate limits, short-lived
login state and the realtime event bus).

### Configuration

Everything is configured through environment variables prefixed with `TERMOSO_`
(nested sections use `__`, e.g. `TERMOSO_S3__BUCKET`) or a TOML file pointed to
by `TERMOSO_CONFIG`. See [`deploy/.env.example`](deploy/.env.example) for the
annotated list; the source of truth is `crates/termoso-server/src/config.rs`.

| Variable | Purpose |
|---|---|
| `TERMOSO_MASTER_KEY` | **required** — 32 random bytes, base64 (`openssl rand -base64 32`) |
| `TERMOSO_DATABASE_URL`, `TERMOSO_REDIS_URL` | backing services |
| `TERMOSO_PUBLIC_URL`, `TERMOSO_WEB_URL` | URLs used in e-mails and OAuth redirects |
| `TERMOSO_ADMIN_EMAILS` | comma-separated e-mails that get the admin role |
| `TERMOSO_CORS_ORIGINS` | browser origins allowed to call the API |
| `TERMOSO_TRUST_PROXY` | honour `X-Forwarded-For` from your reverse proxy |
| `TERMOSO_S3__*` | S3-compatible storage for session logs (optional) |
| `TERMOSO_SMTP__*` | outgoing e-mail (optional; without it e-mail features are off) |
| `TERMOSO_WEBAUTHN__*` | passkeys (optional) |
| `TERMOSO_SSO__<slug>__*` | OIDC providers; `google`, `github`, `microsoft` presets |
| `TERMOSO_METRICS__ENABLED` | opt-in Prometheus metrics on a private listener |
| `TERMOSO_REDIS_PREFIX` | key prefix when several deployments share one Redis |

### Local development

```bash
docker compose -f deploy/docker-compose.dev.yml up -d --wait   # postgres, redis, minio
export TERMOSO_MASTER_KEY=$(openssl rand -base64 32)
cargo run -p termoso-server
# → http://localhost:8080/docs
```

Defaults connect to `postgres://termoso:termoso@localhost:5432/termoso` and
`redis://127.0.0.1:6379`.

### Tests

```bash
cargo test --workspace
```

Unit tests always run. The integration suite in `crates/termoso-server/tests`
boots a real server against PostgreSQL and Redis (it creates a throw-away
database per run and applies the migrations). It is skipped with a notice when
the services are not reachable; set `TERMOSO_TEST_REQUIRE_SERVICES=1` to make
that a failure (CI does). `TERMOSO_TEST_DATABASE_URL` / `TERMOSO_TEST_REDIS_URL`
override the defaults.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
```

## API overview

All routes live under `/api/v1`. Authenticated routes take
`Authorization: Bearer <token>`.

| Area | Routes |
|---|---|
| auth | OPAQUE `register/{start,finish}`, `login/{start,finish}`, `password/{start,finish}`, recovery-key login, MFA step, device approval, SSO (`sso/providers`, `sso/{provider}/start`, callback), `logout` |
| account | profile, e-mail change/verification, devices, key material, recovery-key rotation, security events, deletion |
| account/mfa | TOTP, WebAuthn, backup codes |
| teams | teams, members & roles, invites |
| vaults | personal & team vaults, members, sealed vault keys, key rotation, pending keys |
| sync | `push` / `pull` of encrypted entities with per-vault cursors, version conflicts and tombstones |
| history | encrypted command / connection history |
| logs | session-log upload via pre-signed multipart URLs, listing, deletion |
| ws | realtime notifications (vault changed, session revoked, account updated) |
| admin | users (disable, reset MFA, revoke sessions), teams, server settings, stats, e-mail test |

## License

[AGPL-3.0-or-later](LICENSE). Run it, change it, host it — and keep it free for
whoever you host it for.
