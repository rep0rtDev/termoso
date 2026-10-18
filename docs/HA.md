# Running Termoso highly available

The API is stateless: every replica serves every request, sessions live in
PostgreSQL, short-lived state (login handshakes, codes, rate limits) and
realtime fan-out live in Redis, session logs in S3. Running more than one
replica is a matter of putting a balancer in front and making the stateful
pieces redundant.

```
                 ┌──────────────┐
   clients ────▶ │ LB / proxy   │ ── /readyz per replica
                 └──────┬───────┘
          ┌─────────────┼─────────────┐
   ┌──────▼─────┐ ┌─────▼──────┐ ┌────▼───────┐
   │ api #1     │ │ api #2     │ │ api #N     │   ghcr.io/termoso/termoso-server
   └──────┬─────┘ └─────┬──────┘ └────┬───────┘
          └─────────────┼─────────────┘
     ┌──────────────────┼───────────────────┐
┌────▼──────┐    ┌──────▼──────┐     ┌──────▼──────┐
│ PostgreSQL│    │ Redis       │     │ S3          │
│ primary + │    │ Sentinel or │     │ versioned   │
│ standby   │    │ managed     │     │ bucket      │
└───────────┘    └─────────────┘     └─────────────┘
```

## What is stored where

| Store | Holds | Loss means |
|---|---|---|
| PostgreSQL | accounts, OPAQUE records, teams, roles, policies, vault ciphertext, audit log, webhook outbox, SCIM state, sessions | the deployment — back it up (below) |
| Redis | login handshakes, MFA/approval codes, rate-limit counters, presence, pub/sub between replicas | nobody is logged out; in-flight logins and codes must be redone; no realtime updates until it is back |
| S3 | encrypted session logs | logs; nothing else references them |
| `TERMOSO_MASTER_KEY` | seals server-side secrets (SSO client secrets, webhook secrets, SCIM tokens, SMTP credentials) | the database is only partially usable — the key is part of every backup |

## Readiness and draining

* `GET /healthz` — the process is up.
* `GET /readyz` — `200 {"status":"ready","pg":true,"redis":true,"s3":true,"draining":false,"version":"…"}`
  or `503 {"status":"not_ready", …}` naming the failing dependency. PostgreSQL
  and Redis are probed with a 1 s timeout, S3 (when configured) with 2 s.
* On `SIGTERM` a replica flips `draining` to `true` (so `/readyz` answers 503),
  waits `TERMOSO_DRAIN_SECS` (default 3 — set it to one or two probe
  intervals of your balancer), then stops accepting connections and lets
  in-flight requests finish. WebSocket clients reconnect to another replica
  on their own; terminal sessions are not interrupted because the server
  never carries SSH traffic.

Point the balancer's health check at `/readyz` and its interval below
`TERMOSO_DRAIN_SECS`; roll replicas one at a time.

## Background jobs on several replicas

Periodic work runs on exactly one replica at a time through PostgreSQL
session advisory locks keyed by job name (`pg_try_advisory_lock`); the
others skip the tick. A replica that dies mid-pass releases the lock with
its connection.

| Job | Interval | Lock |
|---|---|---|
| team activity digests | 5 min | `team-digest` |
| audit retention | 6 h | `audit-retention` |
| SCIM account retention | 6 h | `scim-retention` |
| webhook delivery | 5 s, every replica | rows claimed with `FOR UPDATE SKIP LOCKED` — parallel by design |

Intervals carry ±10 % jitter so replicas do not tick in step. Prometheus:
`termoso_job_runs_total{job}` and `termoso_job_seconds{job}`.

Migrations run on start under a lock; several replicas starting at once is
safe, the first one applies them and the rest wait.

## PostgreSQL

* Any managed PostgreSQL ≥ 16 (18 in the Compose stack). Use
  `TERMOSO_DATABASE_URL=postgres://…?sslmode=verify-full&sslrootcert=/path/ca.pem`
  for managed services.
* Pool: `TERMOSO_DATABASE_MAX_CONNECTIONS` per replica (default 20). Size the
  server for `replicas × pool + 10`.
* PgBouncer: the server uses prepared statements, so run PgBouncer in
  `pool_mode = session` or connect directly. `transaction` mode breaks
  queries after the first failover of a pooled connection.
* Read replicas are not used; all traffic goes to the primary. A hot standby
  with automatic promotion (Patroni, managed HA) is the right level of
  redundancy.

## Redis

`TERMOSO_REDIS_URL` accepts:

* `redis://host:6379[/db]` — plain;
* `rediss://host:6380` — TLS (system trust store);
* `redis+sentinel://[:password@]s1:26379,s2:26379,s3:26379/mymaster[?db=0&sentinel_password=…]`
  (`rediss+sentinel://` for TLS to both Sentinel and Redis) — the master is
  resolved at start and re-checked every 10 s; after a failover the server
  switches to the new master without a restart. Pub/sub subscribers reconnect
  through the same lookup.

Redis is hot state, not the source of truth: with Redis down, sync and
terminals keep working, logins/codes fail until it is back, realtime
updates between replicas pause. Set `maxmemory-policy noeviction` and
persistence as in the Compose file; managed Redis with replication is
enough.

## Object storage

Any S3-compatible store. Enable bucket versioning and a lifecycle rule
instead of backing up session logs separately — nothing in the database
depends on their presence.

## Backups and restore

`termoso-server backup|restore|backup-check` (shipped in
`ghcr.io/termoso/termoso-backup`, which also carries `pg_dump`/`pg_restore`
18 and `age`):

```bash
# nightly, e.g. from cron or a systemd timer
docker compose -f deploy/docker-compose.yml --profile backup run --rm backup
# → ./backups/termoso-20261018T020000Z.dump (+ .manifest.json)

# encrypted at rest: generate `age-keygen -o backup-identity.txt`,
# put the public key in .env as TERMOSO_BACKUP_AGE_RECIPIENT=age1…
# → ./backups/termoso-….dump.age
```

The manifest records the server version, the highest applied migration,
the first 16 hex characters of SHA-256 over the master key, row counts of
`users`/`teams`/`vaults`/`entities`, the SHA-256 and size of the file.

`backup-check --from <file>` verifies the checksum and that *this* server
could restore the file (schema not newer, same master key) without touching
the database — run it after every backup and from the restore drill.

`restore --from <file> --yes [--age-identity key.txt] [--force-key]`
refuses a dump from a newer schema ("upgrade the image first") or a
different master key (a restored database whose secrets cannot be opened
is worse than no restore; `--force-key` overrides for forensics), then runs
`pg_restore --clean --if-exists`, applies migrations newer than the dump,
and compares row counts with the manifest (non-zero exit on mismatch).
Stop or drain the API before restoring.

Bare metal: `TERMOSO_PG_DUMP`, `TERMOSO_PG_RESTORE`, `TERMOSO_AGE` point at
the binaries if they are not on `PATH`; they must match the server's major
version.

### Restore drill

`deploy/restore-test.sh <backup> [email password]` brings up a throwaway
Compose project, restores the file into it, starts the API on the restored
data, checks `/api/v1/server/info` and (optionally) that a login handshake
starts, then tears everything down. Run it monthly; CI runs a synthetic
version weekly (`.github/workflows/restore-test.yml`).

### Runbook: the server is gone

Target: 30 minutes to a working deployment.

1. New host: install Docker, clone the repo, restore `deploy/.env` (the
   master key!) from your secret store.
2. `docker compose -f deploy/docker-compose.yml up -d postgres redis rustfs --wait`
3. Copy the latest `termoso-….dump[.age]` + manifest into `./backups`;
   `docker compose --profile backup run --rm backup backup-check --from /backups/<file>`.
4. `docker compose --profile backup run --rm backup restore --from /backups/<file> --yes`
5. `docker compose up -d --wait`; point DNS/the balancer at the new host.
6. Session logs: if the bucket moved, update `TERMOSO_S3__*`; old logs stay
   readable as long as the objects exist.

Clients notice nothing beyond the downtime: sessions are in the restored
database, device trust and keys never left the devices.
