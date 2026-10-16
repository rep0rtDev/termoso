---
title: FAQ
description: Short answers to the questions that come up most — pricing, accounts, what the server sees, platforms, and how Termoso relates to Termius.
---

## General

**Is it really free? What is the catch?**
Yes, and there is none. Termoso is AGPL-3.0 software; the apps, the server and the release pipeline are public. There are no plans, no seat limits and no "team" or "business" tiers — a feature either exists for everyone or does not exist. The free cloud at `app.termoso.dev` runs the same server you can run yourself.

**Do I need an account?**
No. Every app starts with a **Local vault** that holds hosts, keys, snippets and known hosts on that device. An account adds sync between your devices and sharing with a team; you can sign in later and move items into your personal vault at any time.

**What does "the software reports to you, not on you" mean in practice?**
No analytics, crash reporting, usage statistics or advertising identifiers in any app, and no "phone home" on start. The [Privacy page](/docs/security/privacy) lists every network connection the apps can make.

**How does it compare with Termius?**
See [Coming from Termius](/docs/getting-started/migrating-from-termius) for the feature map and the import path.

## Security

**What can the server see?**
Your e-mail, your devices, your team memberships, and ciphertext. Host names, usernames, passwords, keys, snippets and known hosts are encrypted on the device with the vault key before they leave it; sign-in uses OPAQUE, so the server never receives your password either. Details in [Encryption overview](/docs/security/encryption).

**I lost my password. Can support reset it?**
No — nobody can, by design. The 24-word recovery key you were given at sign-up is the only way back into an account. Local vaults on your devices stay readable with each device's own lock.

**Where are private keys stored?**
Encrypted in the vault, like any other item; on Android the database key is wrapped by the Android Keystore, on the desktop by your master password. Keys that live in a hardware token (FIDO2) or in your system SSH agent never enter the vault at all — Termoso only stores the public half.

**Does the AI feature send my terminal to someone?**
Only if you turn it on and ask for a suggestion, and then only the prompt you typed plus the context you can see in the dialog. The endpoint is configurable (`TERMOSO_AI__*` on a self-hosted server), so it can be your own model.

## Platforms

**Which platforms are supported?**
Linux (x86_64, aarch64), Windows, macOS (Apple Silicon and Intel), Android (arm64, armv7, x86_64) and iOS (sideloaded). The [download page](/docs/getting-started/download) lists the files per platform.

**Why is the iOS app behind the others?**
It is the youngest: encrypted local vault, host editor and SSH terminal are there; sync, SFTP and port forwarding are not yet. The release notes say when that changes.

**Is Android really not worse than the desktop?**
Feature for feature it is the most complete mobile client in the project: SSH, Mosh, Telnet, local shell with a [full Linux environment](/docs/terminal/linux-on-android), SFTP and WebDAV with system Files integration, port forwarding, serial over USB, FIDO2 over USB/NFC, snippets, terminal sharing, gestures with an on-screen indicator, voice input and image/file paste.

**Does it work on a Chromebook or in an emulator?**
Yes — install the `x86_64` or universal APK. The Alpine Linux environment is hidden on x86_64 because of an Android seccomp rule; Ubuntu works.

## Using it

**Can I use my existing `~/.ssh/config`?**
Import it: **Hosts → New host ▾ → Import… → OpenSSH**. `Host` blocks become hosts (with `ProxyJump`, `IdentityFile`, ports and users resolved), referenced keys are imported, and `known_hosts` is merged. See [Import existing hosts](/docs/getting-started/import).

**Can several people share hosts without sharing passwords?**
Put the hosts in a team vault and give them an [identity](/docs/keychain/identities) or a key that lives in the same vault. Members connect with it; viewers cannot read or export the secret. Rotate the vault key when someone leaves.

**How do I run the same command on ten servers?**
Either a [snippet](/docs/terminal/snippets) with targets — pick hosts or a group and run it in all of them — or open them as panes in one tab and turn on **Broadcast input**.

**Can I keep a session alive when the phone is in my pocket?**
Yes. Sessions run in a foreground service and show up in the notification shade; exclude Termoso from battery optimisation on vendors that are aggressive about background apps. For flaky networks use Mosh, which survives roaming and sleep.

**Where are session recordings kept?**
Encrypted on the device that recorded them. On servers with object storage configured they can also be kept on the server, encrypted before upload. See [Session logs](/docs/hosts/session-logs).

**How do I move everything to another computer?**
Sign in on the new one — or, without an account, export hosts to CSV (**New host ▾ → Export CSV**) and import keys from `~/.ssh`. Recordings and app settings are per device.

## Self-hosting

**What do I need to run my own server?**
Docker (or Podman), PostgreSQL, Redis, optionally an S3-compatible store for session logs, and a publicly trusted TLS certificate. One Compose file brings it up; see the [overview](/docs/self-hosting/overview).

**Can the apps talk to two servers?**
One account at a time per app, plus the Local vault that is always there. Sign out and in to switch.

**Is there an API?**
The [API Bridge](/docs/self-hosting/api-bridge) exposes a Termius-compatible REST API for hosts and groups on self-hosted servers, encrypting locally so the server still only sees ciphertext. The server's own HTTP API is documented in Swagger UI when `TERMOSO_SWAGGER_UI` is on.

**Where do I report a bug?**
[GitHub issues](https://github.com/rep0rtDev/termoso/issues). The [Troubleshooting](/docs/reference/troubleshooting) page covers the common ones first.
