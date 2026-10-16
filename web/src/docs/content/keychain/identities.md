---
title: Identities
description: A username with its password, key, certificate or security key, saved once and reused by any number of hosts and groups.
---

An identity bundles a **username** with the credential that goes with it — a **password**, a **key**, an **SSH certificate**, a **FIDO2 security key** or an **SSH ID** handle, or several of them. Hosts that use the identity pick up changes to it, so rotating a password or a key is a single edit instead of one per host.

## Creating an identity

**Keychain → New key ▾ → New identity** (or the **Identities** tab): give it a label and a username, then attach a credential with **SSH ID, Key, Certificate, FIDO2**. A password can be typed directly or left empty to be asked at connect time and remembered.

![An identity with a key attached](/docs/img/desktop-identities.png)

## Using an identity

In the host details, open **Credentials → Identity** and pick one. The host's own username and password fields are then taken from the identity; clearing the choice turns them back into per-host fields. A host can still override the username while keeping the identity's key — useful when one key serves `deploy` on most machines and `ubuntu` on a few.

## Identities in groups

Set an identity on a group and every host inside it without its own credentials uses it. Together with a group-level jump host this makes a new server in `Production` a matter of typing its address.

## Rotation

To rotate a key for thirty hosts: generate the new key, **Export to host** it to the group, then swap the key in the identity. Every host switches at once; delete the old key when the last server stopped accepting it. Passwords rotate the same way, minus the deployment step.

## Team vaults

Identities in a team vault are shared with the members of that vault. Members with the **Viewer** role can connect with them but cannot see or export the secret; **Editors** can change it. After someone leaves, rotate the vault key and then the credentials in the identity — the former is cryptographic hygiene, the latter is what actually locks them out of the servers.

> [!TIP]
> Name identities after the role, not the person: `deploy (production)`, `postgres (analytics)`. The label is what you see in a host's credentials field a year later.
