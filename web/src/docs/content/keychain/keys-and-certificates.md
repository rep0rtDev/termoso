---
title: SSH keys and certificates
description: Generate, import and deploy keys, attach OpenSSH certificates, and keep private keys encrypted in the vault.
---

## Keys

Open **Keychain** and choose **New key**:

- **Generate** an Ed25519 (recommended), ECDSA (P-256, P-384, P-521) or RSA key;
- **Import** an existing private key file, with its passphrase if it has one;
- add a **public-only** key whose private half stays in your system SSH agent.

Private keys are stored encrypted in the vault like any other item and sync to your devices. A key referenced by several hosts is stored once.

## Deploying a public key

**Export to host…** appends the public key to `~/.ssh/authorized_keys` on a host you can already log in to, creating the file with the right permissions if needed.

## Certificates

Attach an OpenSSH certificate (`*-cert.pub`) to a key and Termoso offers it during authentication. The Keychain shows whether a certificate is valid, expired or not yet valid.

## System SSH agent

Keys that live in KeePassXC, `ssh-agent`, `ssh-add` or Pageant can be used without copying them: import only the public key and Termoso asks the agent to sign. **Agent forwarding** per host passes the agent on to the server.
