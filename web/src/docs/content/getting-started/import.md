---
title: Import existing hosts
description: Bring hosts in from OpenSSH, PuTTY, Termius, cloud providers or a QR code instead of typing them again.
---

## Desktop

Open **Hosts → New host ▾ → Import…** and pick a source:

| Source           | What is imported                                                                                                                    |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| OpenSSH `~/.ssh` | `Host` blocks from `config` (address, user, port, `IdentityFile`, `ProxyJump`, proxies), the keys they reference and `known_hosts`. |
| PuTTY            | Saved sessions, read from the registry on Windows or from a `.reg` export elsewhere.                                                |
| Termius          | The Termius CSV export template.                                                                                                    |

Parsing only produces a **preview**: nothing is written until you pick what to import and a target vault. Hosts that are already in the vault are recognised, so importing the same file twice does not duplicate them. Private keys are read from disk only when you confirm, and passwords found in the source never reach the interface.

### Cloud providers

**New host ▾ → AWS / DigitalOcean / Azure Integration** lists machines from AWS EC2, DigitalOcean or Azure and turns them into a host group that mirrors the account. **Sync now** (or the background refresh) adds new machines, updates addresses that changed and, if you ask, removes hosts the provider no longer lists — anything you set on a host yourself stays. The provider credentials are kept only in this device's encrypted local storage: they are never part of a vault, so they never sync or reach the server.

## Android

Open **Hosts → + → Quick Import**. Paste an `ssh_config` snippet, a list of `user@host:port` lines, pick a file, or press **Scan QR** to read a QR code with the camera. Decoding runs on the device; the camera is only requested when you open the scanner.

![Quick Import on Android](/docs/img/android-quick-import.png)

**Discover on local network** (in the same menu, and under **New host ▾** on the desktop) lists SSH and SFTP servers that advertise themselves over mDNS on your network.

## Shell history

**Settings → Terminal → Import shell history** reads the history file of the remote shell when you connect, so autocomplete can suggest commands you already ran there. It is off by default, and the imported history is kept in memory only for that session.
