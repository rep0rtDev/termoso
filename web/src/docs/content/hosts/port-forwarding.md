---
title: Port forwarding
description: Local, remote and dynamic (SOCKS) tunnels per host, created with a four-step wizard, started from the sidebar or the session, and kept alive automatically.
---

Port forwarding rules belong to a host and live in the vault with it, so they sync like everything else and can be shared through a team vault.

## Rule types

| Type        | Direction                                                      | Typical use                                                          |
| ----------- | -------------------------------------------------------------- | -------------------------------------------------------------------- |
| **Local**   | A port on this machine → a host:port reachable from the server | Reach a database behind the server: `localhost:5433 → 10.0.2.5:5432` |
| **Remote**  | A port on the server → a host:port reachable from this machine | Expose a local dev server to someone on the server's network         |
| **Dynamic** | A SOCKS5 proxy on this machine routed through the server       | Browse an internal network with the browser's proxy pointed at it    |

The destination of a local rule is resolved **on the server**: `localhost:8000` means port 8000 on the server itself, `10.0.2.5:5432` a machine the server can reach.

## Creating a rule

**Port Forwarding → New forwarding** walks through four steps; each one validates before the next.

![Choosing the rule type](/docs/img/desktop-pf-wizard.png)

1. **Type** — local, remote or dynamic.
2. **Listen** — the bind address and port on this machine (local, dynamic) or on the server (remote). The bind address defaults to `127.0.0.1`; use `0.0.0.0` only when other machines should be able to use the tunnel.
3. **Intermediate host** — the saved host the tunnel goes through. Its credentials, jump host and known-host entry are used; nothing is asked twice.
4. **Destination** — host and port as seen from the server, and a **label** for the list.

The result is a card with the rule in one line and the switches to start it.

![A rule, ready to start](/docs/img/desktop-port-forwarding-edit.png)

Rules can also be created from a session: the **Port forwarding** section of the side panel inside a terminal pre-fills the intermediate host.

## Starting and stopping

Double-click a rule, press its play button or use the **⋯** menu to start it. The card then shows the live state: connected, the number of open connections, bytes in and out, and the last error if there was one. Stopping the rule or closing the app ends it.

![An active tunnel with traffic](/docs/img/desktop-port-forwarding.png)

**Auto-start** on a rule starts it when the vault unlocks, so the tunnels you always need are up before you open a terminal. The same flag is honoured by the Android app.

The sidebar page lists every rule across hosts and vaults; the side panel inside a session lists the rules of that host only. Both show the same state.

## Autoreconnect

A tunnel whose SSH connection drops reconnects by itself — after 2, 4, 8, 16, 32 and then every 60 seconds — and resumes listening on the same port. Connections that were open through it are lost (that is TCP); new ones work as soon as the tunnel is back. The card shows the retry countdown.

## Several rules, one connection

Rules through the same host share one SSH connection when they are started together, and a terminal to that host rides on it too. Starting a tunnel does not open a terminal tab.

## Android

The same rules appear under **Port forwarding** in the vault and in the **⋮ → Port forwarding…** menu of a session. The editor has the same fields as the desktop wizard on one screen.

![Port forwarding rule on Android](/docs/img/android-pf-rule.png)

Active tunnels are listed in **Connections** next to terminals and SFTP, with their traffic, and in the notification shade with a **Stop** button. They keep running in the foreground service while the app is in the background, so a database client on the phone can use `localhost:5433` while Termoso is not on screen.

> [!TIP]
> To check a local rule without another program: start it, then open `http://127.0.0.1:<local port>/` in a browser or run `curl -v` against it. The card's connection counter goes up on every request, and the error line shows what the server said when the destination refused.
