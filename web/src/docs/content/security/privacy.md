---
title: Privacy and telemetry
description: Termoso collects nothing. Here is exactly which network connections the apps make.
---

**No analytics, no crash reporting, no usage statistics, no advertising identifiers.** Not opt-out — absent. The source is public, so this is something you can check rather than trust.

## Every connection the apps make

| Connection                                     | When                                                                                                                                                  |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Your SSH, Mosh, Telnet, SFTP and WebDAV hosts  | When you connect to them                                                                                                                              |
| The server you signed in to                    | Only if you signed in; for sync, teams and account settings                                                                                           |
| Update check (desktop)                         | Only when you press **Check now** or chose **Once at startup**                                                                                        |
| Cloud provider APIs (AWS, DigitalOcean, Azure) | Only when you use an import integration                                                                                                               |
| AI suggestions                                 | Only if you turned them on and ask for a suggestion                                                                                                   |
| Cloudflare Turnstile (captcha)                 | Only on sign-up, account recovery and after repeated failed sign-ins, and only on servers whose operator enabled it (the free cloud does — see below) |

Nothing else. There is no "phone home" on start, and no third-party SDK in any app.

## Server side

The server keeps the minimum it needs to sync and to show you your own security events. Metrics are off by default; when an operator enables them they go to a private Prometheus endpoint of that server, never to the project. Logs never contain credentials, session tokens, keys or vault contents.

## The free cloud

The hosted server at `app.termoso.dev` runs the same open-source server as everyone else, with the same zero-knowledge storage. If you prefer not to trust it at all, [run your own](/docs/self-hosting/overview) or stay offline — every feature except sync works without an account.

### The one third party: a captcha

We would rather have none, but a public sign-up form with no bot check is abused within days, and e-mail verification alone does not stop that. So the free cloud asks Cloudflare Turnstile for a token in exactly three moments: creating an account, recovering one, and signing in after several failed attempts from the same address. While the widget is on screen, Cloudflare sees what any website visit shows it — your IP address and browser characteristics — and nothing about your account, hosts or vault; we never see those signals either. The widget never loads inside a signed-in app or during normal use. Self-hosted servers have it off unless the operator adds keys, and nothing else in Termoso depends on it.
