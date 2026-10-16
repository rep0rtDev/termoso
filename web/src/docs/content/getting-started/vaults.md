---
title: Vaults and sync
description: How Termoso stores hosts, keys and snippets, what syncs and how fast, how to move data between vaults, and what the server can and cannot see.
---

Everything you save — hosts, groups, keys, identities, certificates, snippets, port-forwarding rules, workspace templates, known hosts — lives in a **vault**. A vault is encrypted with its own key; switching vaults switches the whole set of data on screen. The vault switcher is the first tab in the title bar on the desktop and the first screen on Android.

![Vaults on Android](/docs/img/android-vault.png)

## Kinds of vaults

| Vault              | Where it lives                              | Who can open it                       | Needs an account |
| ------------------ | ------------------------------------------- | ------------------------------------- | ---------------- |
| **Local vault**    | Only on this device                         | You, on this device                   | No               |
| **Personal vault** | Your devices, through your server           | You, on every signed-in device        | Yes              |
| **Team vault**     | Members' devices, through the team's server | Every member the vault is shared with | Yes              |

The Local vault is always there, even without an account. Signing in adds your personal vault; joining a team adds the team vaults you were given. All of them are shown together; the footer and the vault switcher tell you which one you are looking at, and new items go into the vault that is open.

## Moving data between vaults

Select hosts (or a whole group) and choose **Move to** or **Copy to** in the context menu, then the destination vault. Moving re-encrypts the items with the destination vault's key; the source vault keeps no copy. Keys and identities referenced by a moved host are offered for moving along with it, so the host keeps working.

Typical first step after signing in: select everything in the Local vault, **Move to → Personal vault**.

## What syncs, and how

Each item is encrypted on the device with **XChaCha20-Poly1305** before it leaves. The server stores the ciphertext, a version number and the minimum metadata needed to sync: which vault, which kind of item, when it changed. It never receives host addresses, usernames, passwords or keys in readable form — not even the names of your hosts.

Changes travel over a WebSocket, so other devices see them within a second while they are online. Offline edits are kept and pushed on the next connection; if two devices changed the same item in the meantime, the most recent edit wins and the other version is kept in the item's history for a while.

![Account and sync settings](/docs/img/desktop-account-sync.png)

**Settings → Account & sync** shows the server, the signed-in account, the devices, the last sync and a **Sync now** button, and lets you sign out — which removes the synced vaults from this device and keeps the Local vault.

### What does not sync

Per-device preferences: colour scheme and font, keyboard shortcuts, the window layout, recording settings, the Android key panel layout, cloud-provider credentials used by import integrations, and session recordings (unless server storage is configured). Everything that describes _your servers_ syncs; everything that describes _this device_ stays.

## Locking

**Lock vault** (<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>L</kbd>) drops the keys from memory; the app asks for the master password or biometrics to continue. See [App lock](/docs/security/app-lock) for automatic locking.

## Without an account

Everything works offline: the Local vault is a full vault with the same features — keys, snippets, port forwarding, recordings, workspace templates. You can sign in later and move hosts into your personal vault at any time, and sign out again without losing the Local vault.

> [!NOTE]
> Back up the Local vault by exporting hosts to CSV (**New host ▾ → Export CSV**) and keys from the Keychain, or simply by signing in and moving items to a personal vault — the server keeps ciphertext you can pull onto a new device.
