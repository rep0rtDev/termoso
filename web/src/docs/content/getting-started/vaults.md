---
title: Vaults and sync
description: How Termoso stores hosts, keys and snippets, what syncs, and what the server can and cannot see.
---

Everything you save — hosts, groups, keys, identities, snippets, port-forwarding rules, known hosts — lives in a **vault**. A vault is encrypted with its own key; switching vaults switches the whole set of data on screen.

## Kinds of vaults

| Vault              | Where it lives                              | Who can open it                       |
| ------------------ | ------------------------------------------- | ------------------------------------- |
| **Local vault**    | Only on this device                         | You, on this device                   |
| **Personal vault** | Your devices, through your server           | You, on every signed-in device        |
| **Team vault**     | Members' devices, through the team's server | Every member the vault is shared with |

The Local vault is always there, even without an account. Signing in adds your personal vault; joining a team adds the team vaults you were given.

## Moving data between vaults

Select hosts (or a whole group) and choose **Move to** or **Copy to** in the context menu. Moving re-encrypts the items with the destination vault's key; the source vault keeps no copy.

## What syncs, and how

Each item is encrypted on the device with **XChaCha20-Poly1305** before it leaves. The server stores the ciphertext, a version number and the minimum metadata needed to sync: which vault, which kind of item, when it changed. It never receives host addresses, usernames, passwords or keys in readable form.

Changes travel over a WebSocket, so other devices see them within a second. Offline edits are kept and pushed on the next connection; if two devices changed the same item in the meantime, the most recent edit wins.

## Without an account

Everything works offline: the Local vault is a full vault with the same features. You can sign in later and move hosts into your personal vault at any time.
