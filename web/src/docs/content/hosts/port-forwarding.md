---
title: Port forwarding
description: Local, remote and dynamic (SOCKS) tunnels per host, started from the side panel and kept alive automatically.
---

Port forwarding rules belong to a host and live in the vault with it, so they sync like everything else.

## Rule types

| Type        | Direction                                                      | Typical use                                                          |
| ----------- | -------------------------------------------------------------- | -------------------------------------------------------------------- |
| **Local**   | A port on this machine → a host:port reachable from the server | Reach a database behind the server: `localhost:5433 → 10.0.2.5:5432` |
| **Remote**  | A port on the server → a host:port reachable from this machine | Expose a local dev server to someone on the server's network         |
| **Dynamic** | A SOCKS5 proxy on this machine routed through the server       | Browse an internal network with the browser's proxy pointed at it    |

## Starting and stopping

Open **Port forwarding** in the sidebar for an overview of every rule across hosts, or the **Port forwarding** section of the side panel inside a session for the rules of that host. Each rule has a start/stop switch and a live status.

![Port forwarding in the side panel](/docs/img/desktop-port-forwarding.png)

## Autoreconnect

A forwarding that loses its SSH connection reconnects automatically and resumes listening. Stopping the rule or closing the app ends it.

## Android

The same rules appear under **Port forwarding** in the vault and can be started from the host menu; active tunnels keep running in the foreground service while the app is in the background.
