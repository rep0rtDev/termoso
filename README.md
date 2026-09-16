# Termoso

A free, open-source, self-hostable SSH/SFTP workstation in the spirit of Termius:
Rust backend, Tauri desktop clients, React Native mobile clients and a web cabinet.
Every feature is available to every user — there are no tiers, seats or paywalls.

Termoso follows one rule: **software reports to you, not on you**. There is no
telemetry, no analytics, no crash reporting and no "phone home". The only metrics
that exist are opt-in Prometheus counters served on a private listener for the
operator running the server.

> Status: **phase 3 — desktop**. The server API is functional and covered by
> integration tests; the web cabinet (account, teams, vaults, admin) is served by
> the same server; the client core (encrypted local store, SSH/SFTP/PTY, sync)
> is done and the Tauri desktop app is being built on top of it. Mobile comes next.

## Repository layout

```
crates/
  termoso-crypto   client + server cryptography (OPAQUE, Argon2id, XChaCha20-Poly1305,
                   X25519 sealed boxes, BIP39-style recovery key)
  termoso-proto    API request/response types shared by server and clients
  termoso-server   the API server (axum + PostgreSQL + Redis + S3)
  termoso-bridge   API Bridge: headless client for your infrastructure that turns a
                   Termius-compatible REST API into encrypted vault entities
  termoso-wasm     termoso-crypto compiled to WebAssembly for the web cabinet
  termoso-core     client engine: encrypted local store, SSH/SFTP/Telnet/PTY, port
                   forwarding, SSH agent, account + sync client
apps/
  desktop/         Tauri 2 desktop app (Linux, Windows, macOS); Rust owns state, storage and
                   sessions, React + MUI is the rendering layer only
  android/         Kotlin + Compose client over termoso-mobile (UniFFI/JNI)
  ios/             SwiftUI client over the same termoso-mobile core (UniFFI Swift, xcodegen);
                   distributed as an unsigned .ipa + AltStore/SideStore source
web/               web cabinet (Vite + React + MUI); all crypto runs in the WASM module
deploy/            Dockerfile, Dockerfile.bridge, docker-compose for production and dev
docs/              API Bridge, releasing, iOS sideloading, benchmarks
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
  key** so a forgotten password does not mean lost data. There is no third
  copy: if both are lost, the encrypted data is unrecoverable, and the only
  way forward is **Start over** — an e-mail-confirmed reset with a 24-hour
  delay (cancellable from the notification) that wipes the old vault and issues
  fresh keys.
* **Step-up re-authentication**: changing the password, recovery key, e-mail
  or second factors, revoking devices, deleting the account or SSH ID keys
  requires re-proving the password (and MFA) within the last 5 minutes
  (`reauth_required`). A stolen bearer token alone cannot lock the owner out,
  and the owner is notified by e-mail of every such change.
* Server-side secrets (OPAQUE server setup, TOTP secrets) are encrypted at rest
  with `TERMOSO_MASTER_KEY`.
* Sessions are opaque bearer tokens bound to a registered device; devices can be
  listed and revoked from any signed-in client and revocation is pushed live.
* MFA: TOTP, WebAuthn/passkeys, e-mail codes and backup codes. New-device login
  approval by e-mail.
* Session logs (terminal recordings) are encrypted client-side and uploaded via
  pre-signed URLs to S3-compatible storage.
* **API Bridge** automation stays zero-knowledge: the bridge container runs in
  your infrastructure, holds the vault keys and pushes only ciphertext — see
  [docs/API_BRIDGE.md](docs/API_BRIDGE.md).
* **AI command suggestions** are off until the account opts in, and the only
  thing that leaves the server is the short request the user typed plus the
  host's OS/shell label — never the terminal buffer, host names, credentials or
  vault contents. The model answers with one command that the client shows for
  review; nothing is executed automatically. Requests are relayed through the
  server with the operator's key (per-account daily quota; prompts and answers
  are not logged). Termoso Cloud uses a model running in a TEE (confidential
  compute: the model sees the text, the hosting operator does not — this is not
  end-to-end encryption). Self-hosted servers point `TERMOSO_AI__*` at their own
  Chutes key or any OpenAI-compatible endpoint (including a local one), or leave
  it unset to have no AI at all.

## Running it

### Production (Docker Compose)

```bash
git clone https://github.com/rep0rtDev/termoso.git && cd termoso
cp deploy/.env.example deploy/.env
$EDITOR deploy/.env              # at least TERMOSO_MASTER_KEY, passwords and public URLs
docker compose -f deploy/docker-compose.yml up -d
```

This starts the server (`:8080`), PostgreSQL, Redis and MinIO. The image bundles
the web cabinet: the API lives under `/api/v1` and everything else on the same
origin serves the cabinet, so one hostname is enough. Put a TLS-terminating
reverse proxy (Caddy, Traefik, nginx) in front of the server and of MinIO (for
pre-signed log uploads) and set `TERMOSO_PUBLIC_URL` and
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
| `TERMOSO_PUBLIC_URL` | URL of the server (API + cabinet), used in e-mails and OAuth redirects |
| `TERMOSO_WEB_URL` | only when the cabinet is hosted on another origin (defaults to `TERMOSO_PUBLIC_URL`) |
| `TERMOSO_SSHID_URL` | optional dedicated origin for SSH ID (e.g. `https://sshid.termoso.example.com`): route it to the same server and handles are served at `/<handle>[/<type>]` there, nothing else; unset = `<TERMOSO_PUBLIC_URL>/sshid/<handle>` |
| `TERMOSO_WEB_DIR` | directory with the built cabinet to serve on `/` (the Docker image sets `/app/web`; unset = API only) |
| `TERMOSO_ADMIN_EMAILS` | comma-separated e-mails that get the admin role |
| `TERMOSO_CORS_ORIGINS` | extra browser origins allowed to call the API (not needed when the cabinet is served by the server) |
| `TERMOSO_TRUST_PROXY` | honour `X-Forwarded-For` from your reverse proxy |
| `TERMOSO_S3__*` | S3-compatible storage for session logs (optional) |
| `TERMOSO_SMTP__*` | outgoing e-mail (optional; without it e-mail features are off) |
| `TERMOSO_WEBAUTHN__*` | passkeys (optional) |
| `TERMOSO_ANDROID_APP_LINKS` | `<package>=<SHA-256 cert fingerprint>` pairs published at `/.well-known/assetlinks.json` so the Android app opens this server's `/invite/…` and `/join/…` links directly (optional) |
| `TERMOSO_SSO__<slug>__*` | OIDC providers; `google`, `github`, `microsoft` presets |
| `TERMOSO_AI__*` | AI command suggestions (optional): `API_KEY` (required to enable), `URL` (OpenAI-compatible base, default `https://llm.chutes.ai/v1`), `MODEL` (default `GLM-4.7-Flash-NVFP4-TEE`), `PROVIDER` (label shown to users), `CONFIDENTIAL` (show the TEE badge; set `false` for ordinary endpoints), `DAILY_QUOTA` (per account, default 50), `MAX_PROMPT_CHARS` (default 500), `TIMEOUT_SECS` |
| `TERMOSO_METRICS__ENABLED` | opt-in Prometheus metrics on a private listener |
| `TERMOSO_REDIS_PREFIX` | key prefix when several deployments share one Redis |

### Local development

```bash
docker compose -f deploy/docker-compose.dev.yml up -d --wait   # postgres, redis, minio, mailpit
export TERMOSO_MASTER_KEY=$(openssl rand -base64 32)
cargo run -p termoso-server
# → http://localhost:8080/docs
```

Defaults connect to `postgres://termoso:termoso@localhost:5432/termoso` and
`redis://127.0.0.1:6379`.

#### Web cabinet

Requires Node 22.12+ and [`wasm-pack`](https://github.com/wasm-bindgen/wasm-pack)
(`cargo install wasm-pack` or `cargo binstall wasm-pack`).

```bash
cd web
npm ci
npm run wasm      # builds crates/termoso-wasm → src/crypto/pkg (generated, git-ignored)
npm run dev       # http://localhost:5173, proxies /api to the server on :8080
```

`npm run wasm` must be re-run after changing `termoso-crypto` or `termoso-wasm`.
`npm run typecheck`, `npm run lint`, `npm run format:check` and `npm run build`
are what CI runs; `npm run build` writes `web/dist`, which the server serves when
`TERMOSO_WEB_DIR=web/dist` is set. The cabinet talks to `/api/v1` on its own
origin only — there are no third-party scripts, fonts or analytics.

#### Desktop app

Requires Node 22.12+ and the [Tauri 2 prerequisites](https://tauri.app/start/prerequisites/)
(Linux: `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libudev-dev`;
macOS: Xcode command line tools).

```bash
cd apps/desktop
npm ci
npm run tauri dev          # Vite on :5174 + the Rust app with hot reload
npm run tauri build        # .deb / .rpm / .AppImage (Linux), NSIS / MSI (Windows), .app / .dmg (macOS)
```

The profile (encrypted SQLite store, settings) lives in the OS data directory
(`~/.local/share/termoso/default` on Linux, `~/Library/Application Support/termoso/default`
on macOS); `TERMOSO_PROFILE_DIR` overrides it. The store key sits in the OS
keychain (Secret Service / Credential Manager / Keychain) with an owner-only
file fallback when no keychain is available. `TERMOSO_LOG` sets the log filter
(stderr only).

On macOS the shortcuts use ⌘ where Linux and Windows use Ctrl (⌘C copies a
selection, ⌘V pastes, ⌘T new tab, ⌘K command palette), so the Control key
reaches the shell unchanged — Ctrl+C is still SIGINT. The stored shortcut
settings are shared across platforms; only the modifier they map to differs.
The window uses the native title bar and traffic lights.

Releases (`.deb`, `.rpm`, AppImage, NSIS, MSI, `.dmg` for Apple Silicon and
Intel) are built and minisign-signed by CI on every `v*` tag. The macOS builds
are ad-hoc signed unless a Developer ID is configured, so the first launch
needs `xattr -cr /Applications/Termoso.app` (or right-click → Open) — see
[docs/RELEASING.md](docs/RELEASING.md#macos-signing-and-gatekeeper). The
built-in updater is off by default: it only contacts
the release feed when you click *Check for updates* (or opt into a startup
check), verifies every download against the public key compiled into the app,
and the feed URL can be pointed at your own HTTPS server. See
[docs/RELEASING.md](docs/RELEASING.md).

#### iOS app

`apps/ios` is a SwiftUI client (iOS 17+) over the same Rust core as Android
(`crates/termoso-mobile` via UniFFI Swift bindings). It is built on macOS:

```bash
cd apps/ios
./build-core.sh                 # Rust → TermosoCoreFFI.xcframework + TermosoCore.swift (simulator)
xcodegen generate               # Termoso.xcodeproj (brew install xcodegen)
open Termoso.xcodeproj
```

There is no App Store listing and no Apple Developer account behind the
project: every `v*` release ships an **unsigned** `termoso-<version>-ios.ipa`
and an AltStore/SideStore source, `termoso-altstore.json`. Add
`https://github.com/rep0rtDev/termoso/releases/latest/download/termoso-altstore.json`
as a source in AltStore or SideStore and it installs the app signed with your
own Apple ID and refreshes the 7-day profile in the background — see
[docs/IOS_SIDELOAD.md](docs/IOS_SIDELOAD.md) for the flow and its limits.

### Tests

```bash
cargo test --workspace
```

Unit tests always run. The integration suite in `crates/termoso-server/tests`
boots a real server against PostgreSQL and Redis (it creates a throw-away
database per run and applies the migrations). MinIO (session logs) and Mailpit
(every email flow: verification, device approval, email MFA, deletion) are
picked up when reachable; SSO runs against an in-process mock OpenID Connect
provider, so no external accounts are needed. Tests needing an unavailable
service are skipped with a notice; set `TERMOSO_TEST_REQUIRE_SERVICES=1` to
make that a failure (CI does). `TERMOSO_TEST_DATABASE_URL`,
`TERMOSO_TEST_REDIS_URL`, `TERMOSO_TEST_S3_ENDPOINT`, `TERMOSO_TEST_SMTP_ADDR`
and `TERMOSO_TEST_MAILPIT_URL` override the defaults.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
```

### Load testing

The load generator is a Rust example rather than a k6 script: OPAQUE and the
vault crypto cannot be reproduced from k6's JavaScript, and a Rust tool reuses
the real client code paths. It registers N users, then each one pushes
encrypted host batches and pulls by cursor, printing latency percentiles and
status counts. Nothing is reported anywhere but stdout.

```bash
cargo run -p termoso-server --release --example loadtest -- \
  --url http://127.0.0.1:8080 --users 20 --duration 30 --batch 20
```

## API overview

All routes live under `/api/v1`. Authenticated routes take
`Authorization: Bearer <token>`.

| Area | Routes |
|---|---|
| auth | OPAQUE `register/{start,finish}`, `login/{start,finish}`, `password/{start,finish}`, recovery-key login, MFA step, device approval, step-up `reauth/{start,finish}`, `start-over/*`, SSO (`sso/providers`, `sso/{provider}/start`, callback), `logout` |
| account | profile, e-mail change/verification, devices, key material, recovery-key rotation, security events, deletion |
| account/mfa | TOTP, WebAuthn, backup codes |
| teams | teams, members & roles, invites; the owner can delete accounts that were created through the team's invitation (`DELETE /teams/{id}/members/{user}/account`, step-up + audit); activity log (`GET /teams/{id}/audit`) and its opt-in per-admin e-mail digest (`GET/PUT /teams/{id}/digest` daily/weekly, `POST /teams/{id}/digest/send`; needs SMTP, mailed 07:00 UTC after the period, nothing sent for a quiet period) |
| vaults | personal & team vaults, members, sealed vault keys, key rotation, pending keys |
| sync | `push` / `pull` of encrypted entities with per-vault cursors, version conflicts and tombstones |
| history | encrypted command / connection history |
| logs | session-log upload via pre-signed multipart URLs, listing, deletion |
| ws | realtime notifications (vault changed, session revoked, account updated) |
| account/ai, ai | AI opt-in + status (`GET/PUT account/ai`), `POST ai/command` → one suggested command (never executed) |
| account/bridges | API bridges: create with sealed vault keys, list, re-seal after rotation, revoke; `bridge/me` for the bridge itself |
| admin | users (disable, reset MFA, revoke sessions), teams, server settings, stats, e-mail test |

## License

[AGPL-3.0-or-later](LICENSE). Run it, change it, host it — and keep it free for
whoever you host it for.
