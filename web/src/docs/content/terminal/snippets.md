---
title: Snippets
description: Saved commands and scripts with variables, grouped into packages and run into one or many sessions.
---

A snippet is a command or a whole script saved in the vault, so it syncs and can be shared through a team vault.

## Creating snippets

Open **Snippets** in the sidebar and press **New snippet**. Give it a name, the script, and optionally a **package** to group related snippets (`Docker`, `Postgres`, `Tuning`…). Snippets without a package are listed under **Unpackaged**.

## Variables

Write placeholders as `{{name}}` in the script. Running the snippet asks for each value once, so one snippet covers every service, branch or path:

```bash
sudo systemctl restart {{service}} && journalctl -u {{service}} -n 50
```

## Running

- From the side panel of a session: click a snippet to paste it, or run it with a trailing newline.
- From the Snippets page: pick one or more hosts and run the snippet in all of them at once.
- From autocomplete: snippets show up in suggestions as you type.

A host can also have a **startup snippet** that runs right after it connects.

## Android

Snippets are in the vault and in the terminal's key panel; tapping one pastes it into the active session.
