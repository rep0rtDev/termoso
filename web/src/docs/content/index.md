---
title: What is Termoso
description: An open-source SSH client for desktop and mobile with end-to-end encrypted vaults, a free cloud you can replace with your own server, and no telemetry.
---

Termoso is an SSH client built around one rule: **the software reports to you, not on you.** It covers the daily work around remote machines — terminal, SFTP, keys, port forwarding, snippets — on Linux, Windows, macOS, Android and iOS, and keeps everything it stores encrypted on your device.

## Basic concepts

### Vaults

A vault is the unit of storage. Every host, key, snippet and known host lives in exactly one vault. The **Local vault** exists on each device and never syncs. A **personal vault** syncs between the devices signed in to the same account. A **team vault** is shared with the members of a team. Each vault is encrypted with its own key, and the server only ever stores ciphertext.

### Hosts and groups

A host is a saved connection: address, port, protocol, credentials, jump host, proxy and terminal preferences. Groups nest and pass settings down to the hosts inside them; tags cut across groups.

### Keychain

Keys, certificates, identities and FIDO2 security keys are stored once in the Keychain and referenced from hosts, so rotating a key means editing one entry.

## Where things run

|                                     | Desktop | Android           | iOS     | Web cabinet |
| ----------------------------------- | ------- | ----------------- | ------- | ----------- |
| Terminal (SSH, Mosh, Telnet, local) | ✓       | ✓                 | SSH     | —           |
| Linux userland for the local shell  | your OS | ✓ Alpine / Ubuntu | —       | —           |
| SFTP and WebDAV                     | ✓       | ✓                 | —       | —           |
| Port forwarding                     | ✓       | ✓                 | —       | —           |
| Serial                              | ✓       | ✓ (USB)           | —       | —           |
| Keychain, snippets, known hosts     | ✓       | ✓                 | partial | —           |
| Session recording                   | ✓       | ✓                 | —       | —           |
| Terminal sharing                    | ✓       | ✓                 | —       | join links  |
| Teams and account settings          | ✓       | ✓                 | —       | ✓           |

The web cabinet manages your account, devices, security settings and teams. By design it never shows the contents of a vault: hosts are decrypted only inside the apps.

![The desktop app](/docs/img/desktop-hosts.png)

## Where to go next

- New here: [Quick start](/docs/getting-started/quick-start), then [Import existing hosts](/docs/getting-started/import).
- Switching clients: [Coming from Termius](/docs/getting-started/migrating-from-termius).
- On a phone: [Mobile terminal](/docs/terminal/mobile), [Compose, voice and AI agents](/docs/terminal/compose-and-ai), [Linux on Android](/docs/terminal/linux-on-android).
- Running it for a team: [Self-hosting](/docs/self-hosting/overview), [Teams](/docs/teams/teams-and-roles), [Encryption overview](/docs/security/encryption).
- Something broke: [Troubleshooting](/docs/reference/troubleshooting) and the [FAQ](/docs/reference/faq).

> [!NOTE]
> Termoso is free software under AGPL-3.0. There are no plans, seats or paid features — the same build serves one person and a whole team.
