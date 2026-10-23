---
title: Snippets
description: Saved commands and scripts with variables, grouped into packages, bound to hosts or groups as targets, and run into one or many sessions.
---

A snippet is a command or a whole script saved in the vault, so it syncs between devices and can be shared through a team vault.

## Creating snippets

Open **Snippets** in the sidebar (<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd>) and press **New snippet**. Give it a **label**, the **script**, and optionally a **package** to group related snippets (`Docker`, `Postgres`, `Tuning`…). Snippets without a package are listed under **Unpackaged**. Scripts can be any length; a trailing newline decides whether the last line runs or waits at the prompt.

![Snippet editor](/docs/img/desktop-snippet-editor.png)

## Variables

Write placeholders as `{{name}}` in the script. Running the snippet asks for each value once — the same name used twice is asked once — so one snippet covers every service, branch or path:

```bash
sudo systemctl restart {{service}} && journalctl -u {{service}} -n 50
```

Values are not saved with the snippet; if a value is the same every time, it is not a variable.

## Targets

A snippet can carry **targets**: the hosts and groups it is meant for. **Add targets** opens a picker over the vault; a group target expands to every host inside it at run time, so a `Production` group picks up new hosts automatically.

![Picking targets](/docs/img/desktop-snippet-targets.png)

Targets are shown in the right pane when the snippet is selected, and **Run** uses them as the default selection.

![Snippets with their targets](/docs/img/desktop-snippets.png)

## Running

**Run** (the play button) opens the run dialog: fill in the variables, then tick where to run — any of the hosts that are already connected in a tab, and the targets, which are connected in a new tab if they are not open. The command is sent to every selected session at once, each in its own terminal.

![Run dialog with a variable and a target](/docs/img/desktop-snippet-run.png)

Other ways to run a snippet:

- **From the side panel of a session** — <kbd>Ctrl</kbd>+<kbd>.</kbd> opens it; click a snippet to paste it at the prompt, or use its menu to run it with <kbd>Enter</kbd>.
- **From autocomplete** — snippets show up in suggestions as you type their label or the beginning of the command; <kbd>Tab</kbd> inserts it.
- **From the host** — the host's context menu has **Run snippet…**, which connects and runs in one step.
- **As a startup snippet** — set on a host or a group, it runs right after the shell is ready: `cd /srv/app && source .env`, a `tmux attach`, an agent start.

## Quiet run

**Run quietly…** (in the snippet menu) runs the snippet on several hosts without opening a terminal: Termoso connects to each host, runs the expanded script over a separate `exec` channel and collects stdout, stderr and the exit code into one table. Pick the hosts (the snippet's targets are preselected), fill in the variables, and watch the rows fill in as hosts finish; expand a row for the full output, **Copy summary** puts everything into the clipboard as text.

It is the right tool for `uptime`, `df -h`, `systemctl is-active {{service}}` across a fleet — read-only checks whose result you want side by side, not in ten tabs. Hosts run eight at a time on desktop (four on Android), each has two minutes for connect plus command, output is kept up to 64 KB per stream. Password, passphrase and host-key prompts appear as usual; a host that fails to connect is shown with the error and does not stop the others. There is no interactive shell behind the command, so anything that needs a TTY (`sudo` with a password prompt, `top`) will not work there — run it in terminals instead.

On Android the same **Run quietly** is in the long-press menu of a snippet.

## Running on many hosts safely

Running a destructive command on ten servers deserves a pause, so a few habits help:

- Put the check first: `systemctl status {{service}}` as one snippet, the restart as another.
- Use targets for the fleet and keep one-off hosts out of them.
- Broadcast input (in a split tab) shows every server's reaction side by side, which is easier to read than ten tabs. See [Tabs, splits and workspaces](/docs/terminal/tabs-and-splits#broadcast-input).

## Android

Snippets are in the vault (**Snippets** in the vault menu) and in the terminal's expanded key panel under **{ }**; tapping one pastes it into the active session, with the variable prompt first if it has any. The **⋮** session menu runs a snippet on the current host, and a host's menu runs one on that host from the list.
