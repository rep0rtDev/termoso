# Self-hosting Termoso

This guide covers running the Termoso server for real users: what each piece
does, how to put it on the internet, and how to keep it alive. For a
five-line quick start see the [README](../README.md#self-hosting); for how
the pieces fit together internally see [ARCHITECTURE.md](ARCHITECTURE.md).

Two equivalent packagings ship in [`deploy/`](../deploy):

| | Docker Compose | Podman Quadlet |
|---|---|---|
| File(s) | `deploy/docker-compose.yml` | `deploy/quadlet/*.container`, `.network`, `.volume` |
| Runtime | Docker Engine + Compose v2, or `podman compose` | Podman 5.x or 6.x driven by systemd (4.4 is the floor; 5.0+ for health-gated start-up, 5.6+ for `podman quadlet install`) |
| Lifecycle | `docker compose up/down/pull` | `systemctl start/stop`, `podman auto-update` |
| Good for | any Linux host, quickest path | servers that already manage everything through systemd, rootless deployments |

Both use the same images, the same `.env` and produce the same running
system. Pick one; do not run both against the same volumes.

`deploy/docker-compose.dev.yml` is **not** a deployment: it starts only the
backing services with fixed passwords for `cargo run`/`cargo test`.

## What runs

| Service | Image | State | Purpose |
|---|---|---|---|
| `api` | `ghcr.io/rep0rtdev/termoso-server` | none | REST + WebSocket API under `/api/v1`, web cabinet on `/`, SSH ID handles, Android asset links. Runs migrations at start. |
| `postgres` | `postgres:18-alpine` | **volume, back it up** | accounts, devices, teams, vaults, encrypted entities, audit log, avatars |
| `redis` | `redis:8-alpine` | volume (disposable) | login handshakes, MFA/approval codes, session cache, rate limits, cross-replica event fan-out |
| `minio` | `quay.io/minio/minio:RELEASE.2025-09-07…` | volume | client-encrypted session logs, reached by clients through pre-signed URLs. MinIO stopped publishing community container images after this tag; newer releases are source-only (`make docker`). Any S3-compatible store is a drop-in replacement — see [Scaling](#scaling). |
| `bridge` (optional) | `ghcr.io/rep0rtdev/termoso-bridge` | none | Termius-compatible REST API for automation — see [API_BRIDGE.md](API_BRIDGE.md) |
| `caddy` (Compose `proxy` profile) | `caddy:2-alpine` | volume (certificates) | TLS termination for the API and MinIO |

Everything the server stores about vault contents is ciphertext the server
cannot open; what it *can* read is listed in the README's
[security model](../README.md#security-model). Losing Redis logs nobody out
permanently — pending logins and MFA codes are lost, sessions are re-read from
PostgreSQL. Losing PostgreSQL loses the deployment. Losing MinIO loses
recorded session logs only.

### Ports

All published ports bind to `127.0.0.1` by default (`TERMOSO_LISTEN` in
`.env` for Compose, `PublishPort=` in the Quadlet units). Nothing is reachable
from other machines until you either put a reverse proxy on the same host or
deliberately change the bind address.

| Port | Service | Expose? |
|---|---|---|
| 8080 | API + cabinet | through your reverse proxy as `TERMOSO_PUBLIC_URL` |
| 9000 | MinIO S3 | through your reverse proxy as `TERMOSO_S3__PUBLIC_ENDPOINT` (only if session logs are used) |
| 9001 | MinIO console | no; use an SSH tunnel |
| 8081 | API Bridge | no; it is an unauthenticated-by-design local API guarded by one shared key |
| 9090 | Prometheus metrics (`TERMOSO_METRICS__*`) | no |
| 80/443 | Caddy (`proxy` profile) | yes, from the internet |

## Configuration

The server reads `TERMOSO_*` environment variables (nested sections use
`__`); the complete annotated list is [`deploy/.env.example`](../deploy/.env.example)
and the source of truth is
[`crates/termoso-server/src/config.rs`](../crates/termoso-server/src/config.rs).
The server validates the configuration at start and refuses to run with an
invalid `TERMOSO_MASTER_KEY`, a non-URL `TERMOSO_PUBLIC_URL`, a partial SMTP
block or an unknown SSO kind.

Minimum for a useful instance:

```dotenv
TERMOSO_MASTER_KEY=            # openssl rand -base64 32 — back it up with the database
TERMOSO_PUBLIC_URL=https://termoso.example.com
POSTGRES_PASSWORD=             # openssl rand -hex 24
MINIO_ROOT_USER=termoso
MINIO_ROOT_PASSWORD=           # openssl rand -hex 24
TERMOSO_S3__PUBLIC_ENDPOINT=https://s3.termoso.example.com
TERMOSO_ADMIN_EMAILS=you@example.com
TERMOSO_SMTP__HOST=… PORT=… USERNAME=… PASSWORD=… SECURITY=starttls FROM=…
```

Without SMTP the server still runs, but e-mail verification, device approval
by e-mail, e-mail MFA and team digests are off. Without `TERMOSO_S3__*` the
session-log feature is absent from the clients.

`TERMOSO_MASTER_KEY` wraps the server-side secrets (SSO client secrets, bridge
tokens, WebAuthn state). A database restored with a different master key is
unusable — store the key next to the backups.

### Reverse proxy and TLS

The API and cabinet share one origin; the cabinet is a single-page app served
with an `index.html` fallback by the server itself, so the proxy only needs
to forward everything, including WebSocket upgrades, to `api:8080` and set
`X-Forwarded-For` (the stack sets `TERMOSO_TRUST_PROXY=true`). The API takes
the *last* address in that header — the one your proxy appended — so a client
cannot spoof its IP for rate limits and security events. Never enable
`TERMOSO_TRUST_PROXY` on a server reachable without the proxy in front.

The Compose `proxy` profile does this with Caddy and Let's Encrypt:

```dotenv
TERMOSO_DOMAIN=termoso.example.com          # = host of TERMOSO_PUBLIC_URL
TERMOSO_S3_DOMAIN=s3.termoso.example.com    # = host of TERMOSO_S3__PUBLIC_ENDPOINT
ACME_EMAIL=you@example.com
```

```bash
docker compose -f deploy/docker-compose.yml --profile proxy up -d --wait
```

Both names must resolve to the host and ports 80/443 must be open. The
[`Caddyfile`](../deploy/Caddyfile) is two `reverse_proxy` blocks; adapt it to
nginx or Traefik if you already run one — the only requirements are HTTP/1.1
upgrade support for `/api/v1/ws` and that the storage hostname forwards to
MinIO unchanged (pre-signed URLs are signed for that exact host and path).

Optional second hostname: `TERMOSO_SSHID_URL` serves SSH ID public keys at the
root of a dedicated origin (`https://sshid.example.com/<handle>`), convenient
for `curl … | tee -a ~/.ssh/authorized_keys`. Point another proxy block at
`api:8080` with that hostname.

### Landing page: on, off, or on its own domain

`/` of the cabinet origin is a public landing page by default. Two knobs
change that; both are plain environment variables, there is no admin switch.

| Layout | `.env` | Result |
|---|---|---|
| One origin (default) | `TERMOSO_PUBLIC_URL=https://termoso.example.com` | `/` is the landing, `/login`, `/account`, … the cabinet, `/api/v1` the API |
| No landing | `TERMOSO_LANDING=false` | `/` redirects to `/login` (a signed-in browser continues to the cabinet); nothing else changes |
| Landing on the apex, cabinet + API on a subdomain | `TERMOSO_PUBLIC_URL=https://app.example.com`<br>`TERMOSO_WEB_URL=https://app.example.com`<br>`TERMOSO_LANDING_URL=https://example.com` | `https://example.com/` is the landing, `https://app.example.com/` goes to `/login` |

In the split layout both hostnames point at the same server (one proxy block
with two addresses is enough — with the Compose `proxy` profile set
`TERMOSO_DOMAIN=app.example.com, example.com`); the server tells them apart
by `Host`. On the landing host only `/`, the static files, the health probes
and `/api/v1/server/info` are answered; every other path — `/login`,
`/signup`, `/invite/…`, `/join/…`, the SSO callback, the whole API — is a
`307` to the same path and query on `TERMOSO_WEB_URL`, so links in e-mails,
invitations, multiplayer links and Android App Links keep working whichever
hostname they were typed on. The landing's own buttons already point at the
cabinet origin. `TERMOSO_LANDING_URL` must be a bare origin, must differ
from `TERMOSO_PUBLIC_URL`/`TERMOSO_WEB_URL`, and needs `TERMOSO_WEB_DIR`
(the landing is part of the cabinet bundle) — the server refuses to start
otherwise. Everything else (`TERMOSO_WEBAUTHN__RP_ID`, OAuth redirect URIs,
`assetlinks.json`) stays tied to `TERMOSO_PUBLIC_URL`, i.e. the cabinet.

### WebAuthn, App Links, SSO

* `TERMOSO_WEBAUTHN__RP_ID` must be the registrable domain of
  `TERMOSO_PUBLIC_URL`; keep the `tauri://localhost,http://tauri.localhost`
  origins so the desktop app can use passkeys.
* `TERMOSO_ANDROID_APP_LINKS=<package>=<SHA-256>` publishes
  `/.well-known/assetlinks.json` so invitation and multiplayer links open in
  the Android app; the fingerprint is the one of the APK you distribute.
* SSO providers are configured as `TERMOSO_SSO__<slug>__*`; the redirect URI
  to register with the IdP is `<TERMOSO_PUBLIC_URL>/api/v1/auth/sso/callback`.
  Only OIDC is implemented; a `KIND=saml` provider is rejected at start-up.
* **Use a publicly trusted certificate** (Let's Encrypt is fine). The Rust
  clients and the server's outgoing HTTPS ship the Mozilla root store
  (`webpki-roots`) and do not consult the OS trust store, so a private CA is
  not accepted by the desktop, Android or iOS apps.

## Docker Compose

```bash
git clone https://github.com/rep0rtDev/termoso.git && cd termoso
cp deploy/.env.example deploy/.env && $EDITOR deploy/.env
docker compose -f deploy/docker-compose.yml up -d --wait
curl -fsS http://127.0.0.1:8080/readyz     # → ready
```

`--wait` returns once the API health check (`termoso-server --healthcheck`,
which calls `/readyz`) passes, i.e. after migrations. Profiles:

```bash
docker compose -f deploy/docker-compose.yml --profile proxy  up -d   # + Caddy
docker compose -f deploy/docker-compose.yml --profile bridge up -d   # + API Bridge
```

The bridge needs `TERMOSO_BRIDGE_API_KEY` and the credentials file exported
from the cabinet at `TERMOSO_BRIDGE_CREDENTIALS_FILE` (default
`deploy/bridge.json`). Podman users can run the same file with
`podman compose`.

Pin the release you run with `TERMOSO_VERSION=0.2.0` rather than `latest`;
the tags match [GitHub releases](https://github.com/rep0rtDev/termoso/releases).

### Upgrading

```bash
git pull                                   # picks up compose/.env.example changes
$EDITOR deploy/.env                        # diff against deploy/.env.example
docker compose -f deploy/docker-compose.yml pull
docker compose -f deploy/docker-compose.yml up -d --wait
```

Migrations are forward-only and run by the first API replica that starts;
they take a lock, so several replicas starting at once are safe. Read the
release notes before jumping several minor versions — a release that requires
manual steps says so.

The stack pins PostgreSQL by major version (`postgres:18-alpine`). A PostgreSQL
major upgrade is never automatic: the 18 image refuses to start on a 17 data
directory. Dump with the old image, then restore into the new one:

```bash
docker compose -f deploy/docker-compose.yml exec postgres pg_dump -U termoso -Fc termoso > pre-upgrade.dump
docker compose -f deploy/docker-compose.yml down
docker volume rm termoso_postgres          # or move it aside
# bump the image tag, then
docker compose -f deploy/docker-compose.yml up -d --wait postgres
docker compose -f deploy/docker-compose.yml exec -T postgres pg_restore -U termoso -d termoso --no-owner < pre-upgrade.dump
docker compose -f deploy/docker-compose.yml up -d --wait
```

Stacks created before the move to PostgreSQL 18 mounted the volume at
`/var/lib/postgresql/data`; the 18+ image keeps its data under
`/var/lib/postgresql/<major>/docker`, which is why the volume is now mounted
at `/var/lib/postgresql` (and why `pg_upgrade --link` works in place there).

### Scaling

The API keeps no local state. Run more replicas behind a proxy that
load-balances them (drop the `ports:` mapping of `api` and let the proxy join
the Compose network, or run replicas on several hosts) and they share
sessions through PostgreSQL and realtime events through Redis pub/sub.

PostgreSQL, Redis and MinIO are single instances in this stack;
replace them with managed services by pointing `TERMOSO_DATABASE_URL`,
`TERMOSO_REDIS_URL` and `TERMOSO_S3__*` elsewhere and removing the services
you no longer need. Any S3-compatible store works, including AWS S3
(`TERMOSO_S3__FORCE_PATH_STYLE=false`, no `ENDPOINT`).

## Podman Quadlet

[Quadlet](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html)
turns the unit files in [`deploy/quadlet/`](../deploy/quadlet) into systemd
services at `daemon-reload`. You get `journalctl`, `systemctl status`,
automatic restarts and optional unattended image updates, with no long-running
daemon.

```
termoso.network              private network, containers resolve each other by name
termoso-{postgres,redis,minio}.volume
termoso-{postgres,redis,minio,api}.container
termoso-bridge.container.example   rename to .container to enable the bridge
stack.env.example            in-stack addresses + credentials → /etc/termoso/stack.env
bridge.env.example           bridge key → /etc/termoso/bridge.env
```

The API reads two environment files: `/etc/termoso/env` (your filled-in copy
of `deploy/.env.example`, identical to the Compose `.env`) and then
`/etc/termoso/stack.env` (container addresses; it overrides the database, Redis
and S3 settings just like the Compose file does). Compose interpolates the
passwords into the URLs for you; with Quadlet you write them in `stack.env`
yourself — the example file marks every place.

### Rootful (system services)

On Podman ≥ 5.6 the unit files can be installed with
`sudo podman quadlet install deploy/quadlet/*.{container,network,volume}`
(`podman quadlet list` / `rm` manage them afterwards); the manual copy below
works on every supported version.

```bash
sudo install -d -m 0750 /etc/termoso
sudo install -m 0600 deploy/.env                    /etc/termoso/env
sudo install -m 0600 deploy/quadlet/stack.env.example /etc/termoso/stack.env
sudo $EDITOR /etc/termoso/stack.env                 # passwords (twice each)
sudo cp deploy/quadlet/*.{container,network,volume} /etc/containers/systemd/
sudo systemctl daemon-reload
sudo systemctl start termoso-api                    # pulls deps via Requires=
sudo systemctl status 'termoso-*'
journalctl -u termoso-api -f
```

Generated units cannot be `systemctl enable`d by hand; their
`[Install] WantedBy=default.target` section is honoured by the generator, so
they start at boot as soon as the files are in place. Stop everything with
`systemctl stop termoso-api termoso-postgres termoso-redis termoso-minio`;
remove the files and `daemon-reload` to retire the stack (volumes stay until
`podman volume rm`).

### Rootless (user services)

Rootless Podman gives every container an unprivileged user namespace; port
8080 and above bind fine without root. The unit files are the same, the paths
change:

```bash
mkdir -p ~/.config/containers/systemd ~/.config/termoso
install -m 0600 deploy/.env                      ~/.config/termoso/env
install -m 0600 deploy/quadlet/stack.env.example ~/.config/termoso/stack.env
sed -i "s#/etc/termoso/#$HOME/.config/termoso/#" deploy/quadlet/*.container
cp deploy/quadlet/*.{container,network,volume} ~/.config/containers/systemd/
systemctl --user daemon-reload
systemctl --user start termoso-api
loginctl enable-linger "$USER"     # keep user services running after logout
```

Rootless containers cannot bind ports below 1024, so terminate TLS with a
system-level proxy (or lower `net.ipv4.ip_unprivileged_port_start`) and point
it at `127.0.0.1:8080`. Rootless networking through pasta costs some
throughput on large SFTP-log uploads; that is the only functional difference.
Podman 6 dropped slirp4netns, cgroups v1, iptables and CNI; hosts that still
rely on any of those stay on Podman 5.x (the units do not depend on either).

### Health, restarts and updates

Every container declares a health check; with Podman ≥ 5.0 `Notify=healthy`
makes a unit *active* only once its check passes, so `termoso-api` waits for
PostgreSQL, Redis and MinIO to be genuinely ready. On Podman 4.x the
dependencies are still ordered, but the API may start a few seconds early and
be restarted by systemd (`Restart=always`, 5 s) until the database answers.
Both behaviours converge on a healthy stack.

To upgrade, change the image tag in `termoso-api.container` (and the bridge
file), then `systemctl daemon-reload && systemctl restart termoso-api`. Or
uncomment `AutoUpdate=registry` and enable `podman-auto-update.timer` to let
Podman pull new images and restart the affected units nightly — reasonable
only if you follow a pinned tag such as `0.2`, not `latest`.

Validate edited unit files without touching the system:

```bash
/usr/libexec/podman/quadlet -dryrun -user   # path varies: /usr/lib/podman/quadlet on some distros
```

## Backups and restore

Back up, in this order of importance:

1. **PostgreSQL** — the deployment.
2. **`TERMOSO_MASTER_KEY`** (in `.env`) — without it the database is only
   partially usable.
3. **MinIO data** — session logs, if you care about them.
4. The `.env` file itself.

Redis holds nothing worth keeping.

```bash
# Compose
docker compose -f deploy/docker-compose.yml exec -T postgres \
  pg_dump -U termoso -Fc termoso > termoso-$(date +%F).dump
docker run --rm -v termoso_minio:/data:ro -v "$PWD":/backup alpine \
  tar czf /backup/termoso-logs-$(date +%F).tgz -C /data .

# Quadlet
podman exec termoso-postgres pg_dump -U termoso -Fc termoso > termoso-$(date +%F).dump
podman volume export termoso-minio > termoso-logs-$(date +%F).tar
```

`pg_dump` runs online; taking it nightly from a timer is enough for most
teams. Keep dumps encrypted at rest — they contain e-mail addresses, team
structure and ciphertext that a future key compromise could unlock.

Restore onto an empty stack (stop the API first so it does not race the
restore with migrations):

```bash
docker compose -f deploy/docker-compose.yml up -d postgres --wait
docker compose -f deploy/docker-compose.yml exec -T postgres \
  pg_restore -U termoso -d termoso --clean --if-exists < termoso-YYYY-MM-DD.dump
docker compose -f deploy/docker-compose.yml up -d --wait
```

The API applies any migrations newer than the dump on start, so restoring an
older dump into a newer version works; the reverse does not.

## Monitoring

* `GET /healthz` — process is up. `GET /readyz` — PostgreSQL and Redis
  answer. Both are unauthenticated and cheap.
* `TERMOSO_METRICS__ENABLED=true` exposes Prometheus metrics on a **separate**
  listener (`TERMOSO_METRICS__BIND`, default `127.0.0.1:9090`) that is never
  published by the stack. Scrape it from the host or over a private network.
* Logs go to stdout as text or JSON (`TERMOSO_LOG_FORMAT`). The server never
  logs credentials, session tokens, keys or vault contents; `TERMOSO_LOG`
  takes a `tracing` filter (`info,sqlx=warn` is a sane production level).

## Development stack

```bash
docker compose -f deploy/docker-compose.dev.yml up -d --wait
cargo run -p termoso-server            # http://localhost:8080, Swagger UI at /docs
cargo test --workspace                 # integration tests use the same services
docker compose -f deploy/docker-compose.dev.yml down -v   # wipe
```

It publishes PostgreSQL (`termoso`/`termoso`), Redis, MinIO
(`minioadmin`/`minioadmin`) and [Mailpit](https://mailpit.axllent.org)
(SMTP on 1025, inbox UI on <http://localhost:8025>) on localhost only. Data
persists in named volumes across restarts. Do not expose it, do not reuse its
passwords, and do not point a production `TERMOSO_DATABASE_URL` at it.

## Limitations

* One region, one PostgreSQL: there is no multi-primary or read-replica
  support in the server.
* MinIO in the stack is a single node; use replicated storage for anything
  you cannot afford to lose.
* The API Bridge exposes hosts and groups only, and holds the keys of the
  vaults it was granted — treat its host as part of those vaults' trust
  boundary.
* SAML SSO is not implemented (configured providers of that kind are
  rejected at start-up); OIDC covers Google, Microsoft, GitHub and any
  discovery-capable IdP.
* Private certificate authorities are not supported by the clients (see
  above); the server must present a chain that validates against the Mozilla
  root store.
