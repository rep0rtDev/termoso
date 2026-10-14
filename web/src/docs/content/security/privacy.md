---
title: Privacy and telemetry
description: Termoso collects nothing. Here is exactly which network connections the apps make.
---

**No analytics, no crash reporting, no usage statistics, no advertising identifiers.** Not opt-out — absent. The source is public, so this is something you can check rather than trust.

## Every connection the apps make

| Connection                                     | When                                                           |
| ---------------------------------------------- | -------------------------------------------------------------- |
| Your SSH, Mosh, Telnet, SFTP and WebDAV hosts  | When you connect to them                                       |
| The server you signed in to                    | Only if you signed in; for sync, teams and account settings    |
| Update check (desktop)                         | Only when you press **Check now** or chose **Once at startup** |
| Cloud provider APIs (AWS, DigitalOcean, Azure) | Only when you use an import integration                        |
| AI suggestions                                 | Only if you turned them on and ask for a suggestion            |

Nothing else. There is no "phone home" on start, and no third-party SDK in any app.

## Server side

The server keeps the minimum it needs to sync and to show you your own security events. Metrics are off by default; when an operator enables them they go to a private Prometheus endpoint of that server, never to the project. Logs never contain credentials, session tokens, keys or vault contents.

## The free cloud

The hosted server at `app.termoso.dev` runs the same open-source server as everyone else, with the same zero-knowledge storage. If you prefer not to trust it at all, [run your own](/docs/self-hosting/overview) or stay offline — every feature except sync works without an account.
