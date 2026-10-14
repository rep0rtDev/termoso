---
title: Teams and roles
description: Create a team, invite people and decide who manages it — free for any number of members.
---

A team groups people who share infrastructure. Teams and memberships are managed in the web cabinet or in the apps; the shared hosts live in [team vaults](/docs/teams/team-vaults).

## Creating a team

Sign in to the cabinet, open **Teams** and press **New team**. You become its **Owner**. On self-hosted servers the administrator can restrict team creation to admins.

## Roles

| Team role  | Can do                                                                                      |
| ---------- | ------------------------------------------------------------------------------------------- |
| **Owner**  | Everything an admin can, plus delete the team and transfer ownership. Exactly one per team. |
| **Admin**  | Manage members, invitations and vaults.                                                     |
| **Member** | Use the vaults they were given access to.                                                   |

Team roles decide who manages the team. Access to the data is granted separately, per vault — see [Team vaults](/docs/teams/team-vaults#vault-roles).

## Inviting people

**Invite** sends an e-mail with a link (or gives you the link to pass on). The invitee signs up or signs in, and an admin then grants vault access. Because vault keys are sealed to each member's public key, granting access happens on an admin's device: the server cannot hand out keys on its own.

## Removing members

Removing a member revokes their sessions for the team at once. Rotate the vault keys afterwards (the cabinet offers it) so that anything they might have kept a copy of becomes useless for new data.

## Activity

The team page shows an activity log of membership changes, vault access and shared sessions. Admins can opt in to a daily or weekly e-mail digest.
