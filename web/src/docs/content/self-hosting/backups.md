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

```bash
docker compose -f deploy/docker-compose.yml exec -T postgres \
  pg_dump -U termoso -Fc termoso > termoso-$(date +%F).dump
docker run --rm -v termoso_rustfs:/data:ro -v "$PWD":/backup alpine \
  tar czf /backup/termoso-logs-$(date +%F).tgz -C /data .
```

`pg_dump` runs online; a nightly timer is enough for most teams. Keep the dumps encrypted at rest: they hold e-mail addresses, team structure and ciphertext.

## Restore

Stop the API first so it does not race the restore with migrations:

```bash
docker compose -f deploy/docker-compose.yml up -d postgres --wait
docker compose -f deploy/docker-compose.yml exec -T postgres \
  pg_restore -U termoso -d termoso --clean --if-exists < termoso-YYYY-MM-DD.dump
docker compose -f deploy/docker-compose.yml up -d --wait
```

Restoring an older dump into a newer version works — the API applies the missing migrations on start. The reverse does not.

## Monitoring

- `GET /healthz` — the process is up. `GET /readyz` — PostgreSQL and Redis answer. Both are unauthenticated and cheap.
- `TERMOSO_METRICS__ENABLED=true` exposes Prometheus metrics on a separate listener (`127.0.0.1:9090` by default) that the stack never publishes.
- Logs go to stdout as text or JSON (`TERMOSO_LOG_FORMAT`) and never contain credentials, tokens, keys or vault contents.
