---
title: SSH keys and certificates
description: Generate, import and deploy keys, attach OpenSSH certificates, use keys from your system agent, and keep private keys encrypted in the vault.
---

## Keys

Open **Keychain** and use the **New key ▾** menu:

![New key menu](/docs/img/desktop-keychain-menu.png)

- **Generate key** — an Ed25519 (recommended), ECDSA (P-256, P-384, P-521) or RSA (2048–4096) key, with an optional passphrase and a comment that ends up in the public key line.
- **New key** — paste or import an existing private key file (OpenSSH, PEM, PuTTY `.ppk`), with its passphrase if it has one; the public key is derived.
- **From SSH agent** — add a **public-only** key whose private half stays in your system agent (see below).
- **Generate FIDO2 key** — a key on a security token; see [FIDO2 security keys](/docs/keychain/security-keys).

![Generating a key](/docs/img/desktop-generate-key.png)

Private keys are stored encrypted in the vault like any other item and sync to your devices. A key referenced by several hosts is stored once; delete it and those hosts fall back to password or ask for credentials.

### Key details

Selecting a key shows its type, fingerprint (SHA-256), comment and the hosts that use it. From here you can **copy the public key**, **export** the private key (asks for the vault to be unlocked and, on Android, for biometrics), **change the passphrase**, attach a certificate, or deploy it.

![Key details](/docs/img/desktop-key-details.png)

### Which type

| Type                | Use it when                                                                                |
| ------------------- | ------------------------------------------------------------------------------------------ |
| **Ed25519**         | Always, unless something below applies. Small, fast, no parameters to get wrong.           |
| ECDSA P-256/384/521 | A server or appliance that accepts ECDSA but not Ed25519 (older network gear, some HSMs).  |
| RSA 3072/4096       | Very old servers (pre-2014 OpenSSH) or policies that mandate RSA. Avoid 2048 for new keys. |
| FIDO2 (`sk-`)       | The private key must never be copyable: it lives on the token and needs a touch per login. |

## Deploying a public key

**Export to host…** appends the public key to `~/.ssh/authorized_keys` on a host you can already log in to — with a password, another key or an agent — creating the directory and file with the right permissions (`700`/`600`) if needed, and skipping the line if it is already there.

![Choosing a host to deploy the key to](/docs/img/desktop-export-key.png)

For a fleet, deploy to a group: every host in it is visited in turn and the result is listed per host.

## Certificates

Attach an OpenSSH certificate (`*-cert.pub`) to a key and Termoso offers it during authentication, which is what servers with `TrustedUserCAKeys` expect. The Keychain shows the certificate's principals, serial and validity window, and whether it is **valid**, **expired** or **not yet valid**; an expired certificate is skipped and the plain key is tried instead.

Certificates are vault items too, so a short-lived certificate issued on the desktop is on the phone a second later.

## System SSH agent

Keys that live in `ssh-agent`, KeePassXC, 1Password, Pageant or a GPG agent can be used without copying them: **From SSH agent** lists the agent's public keys, you pick one, and Termoso asks the agent to sign at login. The private key never enters Termoso. The agent is found through `SSH_AUTH_SOCK` on Linux and macOS and the OpenSSH agent named pipe on Windows.

**Agent forwarding** per host passes the agent on to the server (`ssh -A`); it is off by default and should stay off for hosts you do not control.

## Android

The Keychain on the phone has the same keys, generation and import (from a file or the clipboard) and **Export to host**. Private keys are protected by the Android Keystore in addition to the vault key, and exporting one asks for biometrics or the device credential.

![Keys on Android](/docs/img/android-keys.png)

> [!WARNING]
> There is no "forgot passphrase" for a key: a passphrase protects the key with your secret, and Termoso stores the key as you gave it. If you want keys that do not need a passphrase, rely on the vault's encryption and the app lock instead, and generate them without one.
