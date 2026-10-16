---
title: Keyboard shortcuts
description: Every default shortcut of the desktop app, grouped the way the command palette groups them — and how to change any of them.
---

Every action below is also available from the command palette (<kbd>Ctrl</kbd>+<kbd>K</kbd>), so you never have to remember a chord: type a few letters of the action name and press <kbd>Enter</kbd>. The palette shows the current binding next to each action.

On macOS read <kbd>Ctrl</kbd> as <kbd>⌘</kbd> unless noted otherwise; the two exceptions are copy and paste, which are plain <kbd>⌘</kbd>+<kbd>C</kbd> / <kbd>⌘</kbd>+<kbd>V</kbd> there and <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>C</kbd> / <kbd>V</kbd> on Linux and Windows (plain <kbd>Ctrl</kbd>+<kbd>C</kbd> has to reach the shell).

## Changing a shortcut

Open **Settings → Keyboard** (or run **Keyboard shortcuts** from the palette). Click the chord next to an action and press the new combination; **Reset** restores the default. A binding that is already taken shows which action it belongs to before it is reassigned. Shortcuts are stored in the app settings on this device, not in a vault.

![Keyboard settings](/docs/img/desktop-keyboard-shortcuts.png)

## Navigation

| Action                                            | Default                                       |
| ------------------------------------------------- | --------------------------------------------- |
| Command palette                                   | <kbd>Ctrl</kbd>+<kbd>K</kbd>                  |
| Jump to host or tab…                              | <kbd>Ctrl</kbd>+<kbd>J</kbd>                  |
| Go to Hosts                                       | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>H</kbd> |
| Go to SFTP                                        | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>E</kbd> |
| Go to Port Forwarding                             | <kbd>Ctrl</kbd>+<kbd>P</kbd>                  |
| Go to Snippets                                    | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd> |
| Open Settings                                     | <kbd>Ctrl</kbd>+<kbd>,</kbd>                  |
| Lock vault                                        | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>L</kbd> |
| Open documentation                                | <kbd>F1</kbd>                                 |
| Other pages (Keychain, Known Hosts, Logs, About…) | palette only                                  |

## Tabs

| Action                | Default                                                                                                       |
| --------------------- | ------------------------------------------------------------------------------------------------------------- |
| New tab               | <kbd>Ctrl</kbd>+<kbd>T</kbd>                                                                                  |
| New local terminal    | <kbd>Ctrl</kbd>+<kbd>L</kbd>                                                                                  |
| New serial connection | <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>S</kbd>                                                                   |
| Close tab             | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>W</kbd>                                                                 |
| Next tab              | <kbd>Alt</kbd>+<kbd>→</kbd>, <kbd>Ctrl</kbd>+<kbd>Tab</kbd>, <kbd>Ctrl</kbd>+<kbd>PgDn</kbd>                  |
| Previous tab          | <kbd>Alt</kbd>+<kbd>←</kbd>, <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Tab</kbd>, <kbd>Ctrl</kbd>+<kbd>PgUp</kbd> |
| Duplicate session     | palette / tab menu                                                                                            |

The middle mouse button closes a tab; dragging reorders them.

## Panes

| Action                   | Default                                                      |
| ------------------------ | ------------------------------------------------------------ |
| Split right              | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>D</kbd>                |
| Split down               | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Alt</kbd>+<kbd>D</kbd> |
| Close pane               | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Q</kbd>                |
| Toggle focus mode        | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Enter</kbd>            |
| Focus pane left / right  | <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>←</kbd> / <kbd>→</kbd>   |
| Focus pane above / below | <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>↑</kbd> / <kbd>↓</kbd>   |
| Toggle broadcast input   | <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>B</kbd>                  |
| Move pane to new tab     | palette / pane menu                                          |

## Terminal

| Action                | Linux, Windows                                             | macOS                                                   |
| --------------------- | ---------------------------------------------------------- | ------------------------------------------------------- |
| Copy                  | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>C</kbd>              | <kbd>⌘</kbd>+<kbd>C</kbd>                               |
| Paste                 | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>V</kbd>              | <kbd>⌘</kbd>+<kbd>V</kbd>                               |
| Select all            | <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>A</kbd>                | <kbd>⌘</kbd>+<kbd>⌥</kbd>+<kbd>A</kbd>                  |
| Find in terminal      | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>F</kbd>              | <kbd>⌘</kbd>+<kbd>Shift</kbd>+<kbd>F</kbd>              |
| Clear buffer          | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>K</kbd>              | <kbd>⌘</kbd>+<kbd>Shift</kbd>+<kbd>K</kbd>              |
| Zoom in / out / reset | <kbd>Ctrl</kbd>+<kbd>=</kbd> / <kbd>-</kbd> / <kbd>0</kbd> | <kbd>⌘</kbd>+<kbd>=</kbd> / <kbd>-</kbd> / <kbd>0</kbd> |
| Toggle side panel     | <kbd>Ctrl</kbd>+<kbd>.</kbd>                               | <kbd>⌘</kbd>+<kbd>.</kbd>                               |
| Ask AI for a command  | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>A</kbd>              | <kbd>⌘</kbd>+<kbd>Shift</kbd>+<kbd>A</kbd>              |

Selecting text copies nothing by itself; a right-click on a selection copies it, a right-click without a selection pastes. <kbd>Ctrl</kbd>+click (<kbd>⌘</kbd>+click) opens a link under the pointer — both plain URLs in the output and [OSC 8 hyperlinks](/docs/terminal/appearance#hyperlinks).

## Workspace, create, window

| Action                     | Default                                                            |
| -------------------------- | ------------------------------------------------------------------ |
| Toggle split / list view   | <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>M</kbd>                        |
| Save as workspace template | <kbd>Ctrl</kbd>+<kbd>S</kbd>                                       |
| New host                   | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>N</kbd>                      |
| New group, New snippet     | palette / **New host ▾** menu                                      |
| Toggle full screen         | <kbd>F11</kbd> (macOS also <kbd>⌃</kbd>+<kbd>⌘</kbd>+<kbd>F</kbd>) |
| Quit Termoso               | <kbd>Ctrl</kbd>+<kbd>Q</kbd>                                       |

## Inside the terminal

Keys that are not bound to an action go to the shell unchanged, including <kbd>Ctrl</kbd>+<kbd>C</kbd>, <kbd>Ctrl</kbd>+<kbd>D</kbd>, <kbd>Ctrl</kbd>+<kbd>Z</kbd> and the <kbd>Alt</kbd> chords that Emacs-style editing uses. If a program needs a chord that Termoso has taken (say <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>F</kbd> in a TUI), unbind the action in **Settings → Keyboard**; the key then reaches the program.

> [!TIP]
> On Android there is no chord table — the [key panel](/docs/terminal/mobile#key-panel) provides <kbd>Ctrl</kbd>, <kbd>Alt</kbd>, arrows, function keys and a tmux row, and hardware keyboards work with their ordinary shortcuts.
