---
title: Groups and tags
description: Two ways to organise hosts — groups that nest and inherit settings, tags that cut across them — plus sorting, views and bulk actions.
---

## Groups

A group is a folder for hosts and other groups. Settings on a group — username, credentials, jump host, proxy, terminal preferences, startup snippet — apply to every host inside it that does not set its own, so a `Production` group can carry the bastion and the deploy user once and twenty hosts inherit them.

Create one with **New host ▾ → New group**. The group editor has the same fields as a host, minus the address.

![New group](/docs/img/desktop-new-group.png)

Put hosts in it by **dragging a host card onto the group card**, or set **Parent group** in the host details. Groups nest: drag a group onto another group. Opening a group shows its hosts and subgroups with a breadcrumb at the top.

![Hosts inside a group](/docs/img/desktop-group-view.png)

### Inheritance in practice

| Setting on the group         | Effect on hosts inside                                                                 |
| ---------------------------- | -------------------------------------------------------------------------------------- |
| Username `deploy`            | Hosts with an empty username connect as `deploy`                                       |
| Identity or key              | Used by hosts without their own credentials; a host with a password keeps its password |
| Jump host `bastion`          | Every host without its own jump host hops through `bastion`                            |
| Color scheme                 | Hosts without their own scheme take it — a red scheme for `Production` is a classic    |
| Startup snippet, environment | Run / set on every host that does not override them                                    |

Nested groups resolve from the inside out: a host takes its own value, then the nearest group's, then the parent's, and so on.

Cloud integrations (AWS, DigitalOcean, Azure) create a group that mirrors the account and keeps it up to date — see [Import existing hosts](/docs/getting-started/import#cloud-providers).

## Tags

Tags are labels you attach to any number of hosts regardless of their group — `nginx`, `production`, `customer-x`. Type them in the host details; existing tags are suggested. Filter the list with **Filter by tag** next to the view switcher; several tags narrow the list further. Tags are shown as chips on the host card.

Groups answer "where does this host belong", tags answer "what is this host for". A host is in exactly one group but can have any number of tags.

## Sorting and views

Switch between the card grid and a compact list, and sort **Manual** (drag to reorder), by **Name**, **Address**, **Recently edited** or **Recently connected**. The view and sort order are remembered per device.

## Bulk actions

Select several hosts with <kbd>Ctrl</kbd>+click or <kbd>Shift</kbd>+click and the context menu applies to the whole selection:

- **Connect** — one tab per host;
- **Move to** another vault or group, **Copy to** another vault;
- **Add tag**, **Remove tag**;
- **Export CSV** of the selection;
- **Remove**.

Moving a host to another vault re-encrypts it with that vault's key; the source keeps no copy.

## Android

Groups appear as folders at the top of the host list; tap to open, long-press a host for **Move to group**. Tags filter from the search field.
