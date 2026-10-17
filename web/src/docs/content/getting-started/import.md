---
title: Import existing hosts
description: Bring hosts in from OpenSSH, PuTTY, Termius, MobaXterm, SecureCRT, ZOC, cloud providers, a text snippet or a QR code instead of typing them again.
---

## Desktop

Open **Hosts → New host ▾ → Import…** and pick a source:

![Import sources](/docs/img/desktop-import.png)

| Source           | What is imported                                                                                                                                                                                                               |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| OpenSSH `~/.ssh` | `Host` blocks from `config` (address, user, port, `IdentityFile`, `ProxyJump`, `ForwardAgent`, proxies), the keys they reference and `known_hosts`. Wildcard blocks (`Host *`) are applied as defaults, not imported as hosts. |
| PuTTY            | Saved sessions, read from the registry on Windows or from a `.reg` export elsewhere, including the key file and proxy settings.                                                                                                |
| Termius          | The Termius CSV export: hosts, groups and tags. Termius does not export passwords or keys — see [Coming from Termius](/docs/getting-started/migrating-from-termius).                                                           |
| MobaXterm        | A `.mxtsessions` export or `MobaXterm.ini`: SSH and Telnet sessions with their folders, gateway (jump) chains, SOCKS/HTTP proxies, key paths, X11 and agent forwarding. Passwords are stored encrypted inside MobaXterm and are not part of the file. |
| SecureCRT        | The `Config/Sessions` directory (sub-folders become groups), a single session `.ini`, or the XML from *File → Export Settings*: SSH and Telnet sessions, key files, `Session:` firewalls as jump hosts, descriptions as notes. Saved passwords are encrypted with the config passphrase and are skipped. |
| ZOC Terminal     | `HostDirectory.zhd` (in *ZOC Files → Options*): SSH hosts with folders, usernames, key files and memos. Serial entries are listed as skipped. |

Parsing only produces a **preview**: hosts, keys and settings are listed with what will be created, and nothing is written until you pick what to import and a target vault.

![Import preview](/docs/img/desktop-import-preview.png)

- A `ProxyJump` becomes a **jump host** reference to the imported bastion, so chains keep working.
- Hosts that are already in the vault (same address, port and user) are recognised, so importing the same file twice does not duplicate them; changed fields are updated.
- Private keys are read from disk only when you confirm, and go into the Keychain encrypted. A key with a passphrase is asked for it on first use, not during import.
- Passwords found in the source never reach the interface; PuTTY and Termius do not export them anyway, MobaXterm and SecureCRT keep theirs encrypted and the import says so per session.

### Cloud providers

**New host ▾ → AWS / DigitalOcean / Azure Integration** lists machines from AWS EC2, DigitalOcean or Azure and turns them into a host group that mirrors the account — one subgroup per region or project, instance names as host names, public or private address as you choose. **Sync now** (or the background refresh) adds new machines, updates addresses that changed and, if you ask, removes hosts the provider no longer lists — anything you set on a host yourself (credentials, tags, notes) stays.

The provider credentials are kept only in this device's encrypted local storage: they are never part of a vault, so they never sync or reach the server. The hosts the integration created do sync like any other.

### Export

**New host ▾ → Export CSV** writes the hosts of the current vault (or the selection) in the same CSV layout Termius uses, so a Termoso export imports into either client. Passwords and keys are not exported.

## Android

Open **Hosts → + → Quick Import**. Paste an `ssh_config` snippet, a list of `user@host:port` lines (one per line, `ssh user@host -p 2222` works too), pick a file, or press **Scan QR** to read a QR code with the camera. The counter on the button shows how many hosts were recognised before anything is created.

![Quick Import on Android](/docs/img/android-quick-import.png)

Both formats can be mixed in one paste; QR decoding runs on the device and the camera is only requested when you open the scanner. A handy trick for a new phone: `cat ~/.ssh/config | qrencode -t ansiutf8` on the desktop, then scan.

## Discover on local network

**Discover on local network** (in the **+** menu on Android, and under **New host ▾** on the desktop) lists SSH and SFTP servers that advertise themselves over mDNS/DNS-SD on your network — Linux machines with Avahi, macOS with Remote Login on, NAS boxes, Raspberry Pis. Tap one to connect or save it.

## Shell history

**Settings → Terminal → Import shell history** reads the history file of the remote shell when you connect, so autocomplete can suggest commands you already ran there. It is off by default, and the imported history is kept in memory only for that session.
