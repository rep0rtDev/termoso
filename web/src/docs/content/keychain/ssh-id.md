---
title: SSH ID
description: Publish your public keys under a handle so servers can trust a person instead of a file.
---

SSH ID gives your account a public **handle**. The keys you choose are published at a URL that serves plain `authorized_keys` text:

```
https://sshid.termoso.dev/<handle>
```

`curl` and `AuthorizedKeysCommand` get the text; open the same address in a browser and you get a page with the install command for Shell and PowerShell, the key types with the OpenSSH version each needs, fingerprints of the published keys and a short explanation to send to whoever is granting you access. `/<handle>/<type>` lists one type (`ED25519`, `ECDSA`, `RSA`, `ECDSA-SK`, `ED25519-SK`), `/<handle>/all` every type; `/<handle>` alone is ED25519.

## Setting it up

1. Open **Keychain → SSH ID** in the desktop app and pick a handle.
2. Choose which keys to publish: keys of this device, FIDO2 security keys, or both.
3. On a server, trust the handle:

```bash
curl -fsSL https://sshid.termoso.dev/<handle> >> ~/.ssh/authorized_keys
```

or use it in `AuthorizedKeysCommand` to always fetch the current list.

## Rotating keys

Remove a key from SSH ID and it disappears from the published list at once; servers that fetch the list on login stop accepting it immediately.

## Privacy

Only public keys and the handle are published; device keys are listed only while that device is signed in. Self-hosted servers publish at `https://<server>/sshid/<handle>` and can add a dedicated origin (`TERMOSO_SSHID_URL`) such as `https://sshid.example.com/<handle>` — the `/sshid/` path keeps working either way, and browsers opening it are redirected to the dedicated one.
