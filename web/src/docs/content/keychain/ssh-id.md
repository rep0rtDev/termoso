---
title: SSH ID
description: Publish your public keys under a handle so servers can trust a person instead of a file.
---

SSH ID gives your account a public **handle**. The keys you choose are published at a URL that serves plain `authorized_keys` text:

```
https://app.termoso.dev/sshid/<handle>
```

## Setting it up

1. Open **Keychain → SSH ID** in the desktop app and pick a handle.
2. Choose which keys to publish: keys of this device, FIDO2 security keys, or both.
3. On a server, trust the handle:

```bash
curl -fsS https://app.termoso.dev/sshid/<handle> >> ~/.ssh/authorized_keys
```

or use it in `AuthorizedKeysCommand` to always fetch the current list.

## Rotating keys

Remove a key from SSH ID and it disappears from the published list at once; servers that fetch the list on login stop accepting it immediately.

## Privacy

Only public keys and the handle are published. Self-hosted servers can serve SSH ID on a dedicated origin (`TERMOSO_SSHID_URL`) such as `https://sshid.example.com/<handle>`.
