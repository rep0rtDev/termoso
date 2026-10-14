---
title: Identities
description: A username with its password or key, saved once and reused by any number of hosts.
---

An identity bundles a **username** with a **password**, a **key** or both. Hosts that use the identity pick up changes to it, so rotating a password or a key is a single edit instead of one per host.

## Using an identity

In the host details, open **Credentials → Identity** and pick one. The host's own username and password fields are then taken from the identity; clearing the choice turns them back into per-host fields.

## Identities in groups

Set an identity on a group and every host inside it without its own credentials uses it.

## Team vaults

Identities in a team vault are shared with the members of that vault. Members can connect with them; the secret itself stays encrypted with the vault key.
