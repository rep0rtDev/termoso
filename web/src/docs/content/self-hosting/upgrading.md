---
title: Upgrading
description: Move to a new release, pin versions, and handle PostgreSQL major upgrades.
---

## Pin the version

Set `TERMOSO_VERSION` in `deploy/.env` to the release you run (for example `0.7.0`) rather than `latest`. Tags match the [GitHub releases](https://github.com/rep0rtDev/termoso/releases).

## Upgrade

```bash
git pull                                    # compose and .env.example changes
$EDITOR deploy/.env                         # compare with deploy/.env.example, bump TERMOSO_VERSION
docker compose -f deploy/docker-compose.yml pull
docker compose -f deploy/docker-compose.yml up -d --wait
```

Migrations are forward-only and run when the first API replica starts; they take a lock, so several replicas starting together are safe. Read the release notes before skipping several versions — a release that needs manual steps says so.

## Clients

Apps and servers of neighbouring versions work together. Update the server first, then the apps.

## PostgreSQL major versions

The stack pins PostgreSQL by major version. A major upgrade is never automatic — the new image refuses an old data directory. Dump with the old image and restore into the new one:

```bash
docker compose -f deploy/docker-compose.yml exec postgres pg_dump -U termoso -Fc termoso > pre-upgrade.dump
docker compose -f deploy/docker-compose.yml down
docker volume rm termoso_postgres          # or move it aside
# bump the postgres image tag, then
docker compose -f deploy/docker-compose.yml up -d --wait postgres
docker compose -f deploy/docker-compose.yml exec -T postgres pg_restore -U termoso -d termoso --no-owner < pre-upgrade.dump
docker compose -f deploy/docker-compose.yml up -d --wait
```
