---
title: Backups and monitoring
description: What to back up, how to restore, and how to know the server is healthy.
---

## What to back up

In order of importance:

1. **PostgreSQL** — the deployment.
2. **`TERMOSO_MASTER_KEY`** from `.env` — without it the database is only partially usable.
3. **Object storage** — session logs, if you use them.
4. The `.env` file itself.

Redis holds nothing worth keeping.

The `backup` profile ships `termoso-server backup | restore | backup-check` next to `pg_dump`/`pg_restore` 18 and `age`:

```bash
# nightly from cron or a systemd timer
docker compose -f deploy/docker-compose.yml --profile backup run --rm backup
#   → ./backups/termoso-20261018T020000Z.dump + .manifest.json

# verify a file against this server: checksum, schema, master key (no DB access)
docker compose -f deploy/docker-compose.yml --profile backup run --rm backup \
  backup-check --from /backups/termoso-20261018T020000Z.dump
```

Set `TERMOSO_BACKUP_AGE_RECIPIENT=age1…` in `.env` (`age-keygen` makes the pair) for `age`-encrypted dumps; keep the identity file with the master key, not with the dumps. Bare metal: the same subcommands work with `pg_dump` of the server's major on `PATH` (`TERMOSO_PG_DUMP`/`TERMOSO_PG_RESTORE`/`TERMOSO_AGE` override the binaries).

The manifest records the server version, schema version, a master-key fingerprint, row counts and the SHA-256 of the dump. Dumps hold e-mail addresses, team structure and ciphertext — keep them encrypted at rest.

## Restore

Stop the API first so it does not race the restore with migrations:

```bash
docker compose -f deploy/docker-compose.yml stop api
docker compose -f deploy/docker-compose.yml --profile backup run --rm backup \
  restore --from /backups/termoso-YYYYMMDDTHHMMSSZ.dump --yes   # [--age-identity /backups/key.txt]
docker compose -f deploy/docker-compose.yml up -d --wait
```

`restore` refuses a dump from a newer schema (install that image first) or a different master key (`--force-key` only for forensics — the secrets would be unreadable), runs `pg_restore --clean --if-exists`, applies the migrations the dump predates, and exits non-zero when row counts differ from the manifest.

Rehearse: `deploy/restore-test.sh <dump> [email password]` restores into a throwaway Compose project, starts the API on it and checks a login handshake. CI runs a synthetic drill weekly.

## Runbook: the host is gone

Target 30 minutes.

1. New host with Docker; clone the repo; restore `deploy/.env` (the master key!) from your secret store.
2. `docker compose -f deploy/docker-compose.yml up -d postgres redis rustfs --wait`
3. Copy the latest dump + manifest into `./backups`; run `backup-check` on it.
4. `docker compose --profile backup run --rm backup restore --from /backups/<file> --yes`
5. `docker compose up -d --wait`; move DNS / the balancer.

Clients notice nothing beyond the downtime — sessions are in the restored database, keys never left the devices.

## Monitoring

- `GET /healthz` — the process is up. `GET /readyz` — JSON with `pg`, `redis`, `s3`, `draining`; 503 while any is false. Both are unauthenticated and cheap. Several replicas: see [High availability](/docs/self-hosting/ha).
- `TERMOSO_METRICS__ENABLED=true` exposes Prometheus metrics on a separate listener (`127.0.0.1:9090` by default) that the stack never publishes.
- Logs go to stdout as text or JSON (`TERMOSO_LOG_FORMAT`) and never contain credentials, tokens, keys or vault contents.
