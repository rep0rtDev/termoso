---
title: Groups and tags
description: Two ways to organise hosts — groups that nest and inherit settings, tags that cut across them.
---

## Groups

A group is a folder for hosts and other groups. Settings on a group — username, credentials, jump host, proxy, terminal preferences — apply to every host inside it that does not set its own, so a `production` group can carry the bastion and the deploy user once.

Create one with **New host ▾ → New group**; drag hosts in, or set the parent group in the host details.

Cloud integrations (AWS, DigitalOcean, Azure) create a group that mirrors the account and keeps it up to date — see [Import existing hosts](/docs/getting-started/import#cloud-providers).

## Tags

Tags are labels you attach to any number of hosts regardless of their group. Filter the list with **Filter by tag** next to the view switcher.

## Sorting and views

Switch between the card grid and a compact list, and sort **Manual** (drag to reorder), by **Name**, **Address**, **Recently edited** or **Recently connected**.

## Bulk actions

Select several hosts and the context menu applies to the whole selection — for example **Move to** another vault or group, **Copy to**, or **Remove**.
