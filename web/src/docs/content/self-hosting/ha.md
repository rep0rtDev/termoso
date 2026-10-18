---
title: High availability
description: Several API replicas, managed PostgreSQL and Redis, what survives what.
---

The API is stateless: sessions live in PostgreSQL, short-lived state and realtime fan-out in Redis, session logs in S3. Running more than one replica means a balancer in front and redundant stateful pieces behind.

## Readiness and draining

- `GET /healthz` — the process is up.
- `GET /readyz` — `200 {"status":"ready","pg":true,"redis":true,"s3":true,"draining":false}` or `503` naming the failing dependency (1 s probes for PostgreSQL and Redis, 2 s for S3).
- On `SIGTERM` the replica answers 503 on `/readyz` for `TERMOSO_DRAIN_SECS` (default 3), then stops accepting connections and finishes in-flight requests. WebSocket clients reconnect elsewhere; SSH sessions are never routed through the server, so nothing drops.

Point the balancer's health check at `/readyz` with an interval shorter than `TERMOSO_DRAIN_SECS`; roll replicas one at a time.

## Background jobs

Digests, audit retention and SCIM retention run on exactly one replica at a time through PostgreSQL advisory locks (`team-digest` every 5 min, `audit-retention` and `scim-retention` every 6 h, ±10 % jitter). Webhook delivery runs on every replica and claims rows with `SKIP LOCKED`. Metrics: `termoso_job_runs_total{job}`, `termoso_job_seconds{job}`. Migrations run under a lock on start, so replicas may start together.

## PostgreSQL

- Managed PostgreSQL ≥ 16 (18 in the Compose stack); `TERMOSO_DATABASE_URL=postgres://…?sslmode=verify-full`.
- `TERMOSO_DATABASE_MAX_CONNECTIONS` per replica (default 20); size the server for `replicas × pool + 10`.
- PgBouncer only in `pool_mode = session` (the server uses prepared statements).
- No read replicas; a hot standby with automatic promotion is the right redundancy.

## Redis

`TERMOSO_REDIS_URL` takes `redis://`, `rediss://` (TLS) or `redis+sentinel://[:password@]s1:26379,s2:26379/mymaster[?db=0&sentinel_password=…]`. With Sentinel the master is re-resolved every 10 s and the server switches after a failover without a restart. Redis is hot state, not the source of truth: with it down, sync and terminals keep working, logins and codes fail until it is back.

## Object storage

Any S3-compatible store; turn on bucket versioning instead of backing session logs up separately.

## What survives what

| Lost                 | Effect                                                                                                                  |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| one API replica      | nothing — balancer stops routing to it                                                                                  |
| Redis                | no new logins/codes, no realtime fan-out; nobody logged out                                                             |
| S3                   | session logs unavailable; everything else works                                                                         |
| PostgreSQL           | outage — restore from backup ([Backups](/docs/self-hosting/backups))                                                    |
| `TERMOSO_MASTER_KEY` | server-side secrets (SSO, webhooks, SCIM tokens) unreadable; user vaults are unaffected because they never depend on it |

The runbook for losing the whole host is on the [Backups](/docs/self-hosting/backups) page.
