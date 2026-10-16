---
title: Coming from Termius
description: What maps to what, how to move hosts, keys and snippets over in one afternoon, and the few places where Termoso deliberately works differently.
---

Termoso covers the Termius workflow — hosts, groups, tags, keys, identities, snippets with variables, SFTP, port forwarding, team vaults, terminal sharing, mobile gestures — but with every feature in the free build, no per-seat pricing and no telemetry. This page is for people who already know Termius and want the shortest path.

## Vocabulary

| Termius                                   | Termoso                                                                                                |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Personal / Team vault                     | **Personal vault** / **team vault** — same idea; plus a **Local vault** that never leaves the device   |
| Hosts, Groups, Tags                       | Same names; groups inherit settings, tags filter                                                       |
| Keychain → Keys, Identities, Certificates | **Keychain** with keys, certificates, identities, FIDO2 security keys and SSH ID                       |
| Snippets, Snippet packages                | **Snippets** with `{{variables}}` and packages; run on one host or many                                |
| SFTP                                      | **SFTP** two-pane browser; also WebDAV; Android exposes hosts to the system Files app                  |
| Port forwarding                           | Same three rule types (local, remote, dynamic), autoreconnect, start from the sidebar or the session   |
| Terminal sharing                          | **Terminal sharing** with an end-to-end encrypted link                                                 |
| Logs                                      | **Session logs** (recordings, searchable, pin to keep) plus connection history                         |
| Termius Cloud                             | The free cloud at `app.termoso.dev`, or [your own server](/docs/self-hosting/overview) — same software |
| Business / Team plans                     | There are none. Teams, SSO, audit log and the API bridge are in the open-source build                  |

## Moving your data

### 1. Export from Termius

In Termius desktop: **Settings → Export** produces a CSV with hosts, groups and tags (passwords are **not** included by Termius). Private keys have to be exported one by one from the Keychain, or you can reuse the files in `~/.ssh` if that is where they came from.

### 2. Import into Termoso

**Hosts → New host ▾ → Import… → Termius** reads the CSV template and shows a preview: hosts with address, port, username, group path and tags, duplicates already detected. Pick the target vault and confirm.

![Import preview with hosts, a key and settings](/docs/img/desktop-import-preview.png)

For keys, use **Import… → OpenSSH `~/.ssh`** when the keys are there (the `config` file comes along with `ProxyJump` and `IdentityFile` resolved), or **Keychain → New key ▾ → Import key** for individual files. Attach keys to hosts afterwards, or create an [identity](/docs/keychain/identities) once and assign it to a group.

### 3. On the phone

Sign in to the same account and the vault appears. For a phone-only start, **Hosts → + → Quick Import** accepts an `ssh_config` snippet, `user@host:port` lines or a QR code.

## Things that work differently — on purpose

- **Nothing is sent in plain text, ever.** Termius encrypts vault data too, but Termoso goes further: the server also never sees your password (OPAQUE sign-in), host names and usernames are ciphertext, and there is no analytics or crash-reporting SDK in any app. See [Privacy](/docs/security/privacy).
- **Local vault.** You can use Termoso for years without an account. Sync is a choice, not a requirement.
- **Open source, AGPL-3.0.** The server, the apps and the release pipeline are public; the same code runs the free cloud.
- **Updates are manual by default** on the desktop, and the Android APK comes from GitHub, not a store. Both can be switched to "check at startup" or sideloaded through your own feed.
- **Autocomplete is local.** Suggestions come from your history and snippets on the device; the optional AI command suggestion is opt-in and can point at your own model endpoint.

## Not there yet

Honest list of Termius features Termoso does not have today:

- iOS is the youngest app: local vault, host editor and SSH terminal, but no sync, SFTP or port forwarding yet.
- ML-DSA (post-quantum signature) SSH keys — waiting on the Rust SSH stack; the ML-KEM hybrid key _exchange_ is already there.
- A hosted "cloud terminal" — Termoso connects your device to your hosts; it does not run shells for you.

Everything else from the Termius changelog of the last two years — swipe gestures with the arrow indicator, pasting images and files into the terminal, voice typing and the AI-agent compose box, OSC 8 hyperlinks, Nerd Font glyphs, pinned logs, port forwarding in Connections, desktop focus mode, a Linux environment on Android — is in 0.8.0.
