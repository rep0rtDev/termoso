---
title: Encryption overview
description: What is encrypted, with which keys, and what the server can and cannot see.
---

Termoso is built so that the operator of the server — even when that is you — cannot read stored hosts, keys or passwords.

## On the device

Every item in a vault is encrypted with **XChaCha20-Poly1305** using the vault's key before it is stored or sent. The local database on each device is encrypted too; its key is protected by the operating system (Android Keystore, iOS Keychain) or by your master password on the desktop.

## Keys

| Key                       | Purpose                                    | Who holds it                                                          |
| ------------------------- | ------------------------------------------ | --------------------------------------------------------------------- |
| Vault key                 | Encrypts the items of one vault            | Members of the vault, sealed to each member's public key              |
| Account key pair (X25519) | Receives sealed vault keys                 | You; the private key is wrapped with a key derived from your password |
| Recovery key (24 words)   | Second wrapping of the account private key | Only you                                                              |

There is no server-side escrow. Without your password or the recovery key, nobody — including the server operator — can unwrap your keys.

## What the server stores

- Your e-mail, display name, devices and team memberships.
- Ciphertext of every item, with its vault, kind, version and timestamps.
- Sealed copies of vault keys.
- Security events (sign-ins, device approvals) for your account page.

It never stores your password (see [Account security](/docs/security/account)), host addresses, usernames, passwords or private keys in readable form.

## In transit

All traffic to the server uses TLS. SSH sessions are end-to-end between your device and your host; the server is not involved. Shared terminals are encrypted with a per-session key that the relay never sees.

## Post-quantum

SSH connections use the ML-KEM hybrid key exchange when the server supports it, which protects recorded traffic against a future quantum computer.
