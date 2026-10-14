---
title: Team vaults
description: Shared hosts, keys and snippets, encrypted with a key only the members hold.
---

A team vault works like your personal vault, but every member it is shared with sees the same hosts, groups, keys, identities and snippets. Changes sync to everyone within seconds.

## How sharing works

Each vault has its own symmetric key. When a member is given access, an admin's device seals that key to the member's public key and uploads the sealed copy. The server stores sealed copies it cannot open; only the members' devices can.

## Vault roles

| Vault role  | Can do                                                |
| ----------- | ----------------------------------------------------- |
| **Viewer**  | Read and connect; cannot change anything.             |
| **Editor**  | Create, edit and delete items.                        |
| **Manager** | Editor, plus manage the vault's members and settings. |

## Key rotation

After removing someone, rotate the vault key from the vault page. The new key is sealed to the remaining members; until a member's device has received it, the vault shows as **Locked on this device — waiting for a manager to hand you the key**.

## Presence

When presence is on for the team, members can see who is connected to which host of a team vault right now. It is opt-in per team and per user.
