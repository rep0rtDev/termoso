---
title: Tabs, splits and workspaces
description: Several sessions side by side in one tab, focus mode for the one that matters, input broadcast to all of them, and layouts you can save and reopen.
---

## Tabs

Each session opens in its own tab in the title bar. Drag tabs to reorder them; the middle mouse button closes one; <kbd>Ctrl</kbd>+<kbd>Tab</kbd> / <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Tab</kbd> (or <kbd>Alt</kbd>+<kbd>←</kbd>/<kbd>→</kbd>) cycle through them. Right-click a tab (or use its menu) for the session actions:

![Tab menu](/docs/img/desktop-tab-menu.png)

| Action                            | What it does                                                                                                     |
| --------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| **Split right / Split down**      | Open a second session to the same host next to or below this one                                                 |
| **Duplicate session**             | The same host in a new tab                                                                                       |
| **Open SFTP for this connection** | A file browser on the same SSH connection, no second authentication                                              |
| **Share this terminal**           | An end-to-end encrypted link for someone to watch or type — see [Terminal sharing](/docs/teams/terminal-sharing) |
| **Broadcast input to all panes**  | Every keystroke goes to all panes of the tab                                                                     |
| **Find**, **Clear buffer**        | Search the scrollback; wipe it                                                                                   |
| **Save as workspace template**    | Remember this tab's layout and hosts                                                                             |
| **Close others**, **Close**       |                                                                                                                  |

## Splits

A tab can hold up to sixteen panes. Split with the tab menu or <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>D</kbd> (right) / <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Alt</kbd>+<kbd>D</kbd> (down); the new pane connects to the same host. Drag the divider to resize, click a pane to focus it — the focused pane has a highlighted frame — or move focus with <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+arrows.

![Two panes in one tab](/docs/img/desktop-split.png)

Each pane has its own header with the host's OS icon, name and address, and buttons for broadcast, its menu and closing it. A pane's menu can also connect it to a **different** host, or **Move pane to new tab**.

## Focus mode

<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Enter</kbd> (or **Window → Toggle focus mode**) expands the focused pane to fill the tab, hiding the other panes without closing them. The same chord brings the layout back. Use it to read a long log or work in an editor on one of four servers, then return to the overview.

![Focus mode: one pane filling the tab](/docs/img/desktop-focus-mode.png)

Focus mode is per tab; switching to another tab shows that tab's own layout.

## Broadcast input

**Broadcast input to all panes** (<kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>B</kbd>) sends every keystroke to all panes of the tab at once — useful for running the same command on a fleet, or for logging in to several machines at once. The pane headers highlight while broadcast is on; turn it off from the same menu. Pasting is broadcast too, so check the clipboard first.

## Workspaces

**Save as workspace template** (<kbd>Ctrl</kbd>+<kbd>S</kbd>) stores the layout of the current tab — which hosts, in which panes, which sizes — under a name. Open a template from **New tab** to reconnect the whole layout in one step; a saved template is also a quick way to open "the four servers I look at every morning".

Templates live in the vault, so they sync and can be shared through a team vault. On start-up the app can also restore the tabs that were open when it was closed.

**Toggle split / list view** (<kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>M</kbd>) shows the panes of a tab as a vertical list instead of a grid, which suits a narrow window.

## Side panel

<kbd>Ctrl</kbd>+<kbd>.</kbd> or the button on the right of the title bar opens the side panel of the current session: terminal search and per-session settings (scheme, font size), the snippets of the vault, the port-forwarding rules of the host, and the session's command history from shell integration.

## Command palette and shortcuts

<kbd>Ctrl</kbd>+<kbd>K</kbd> (<kbd>⌘</kbd>+<kbd>K</kbd> on macOS) opens the command palette to jump to any action, with its shortcut shown next to it; <kbd>Ctrl</kbd>+<kbd>J</kbd> jumps to a host or tab by name.

![Command palette](/docs/img/desktop-command-palette.png)

Every shortcut can be changed in **Settings → Keyboard** — the full table is under [Keyboard shortcuts](/docs/reference/keyboard-shortcuts).
