---
title: Mobile terminal
description: The Android terminal — full screen, a session bar above the keyboard, a configurable key panel, gestures with an on-screen indicator, and sessions that keep running in the notification shade.
---

## Layout

The terminal uses the whole screen. Above the keyboard sits the **session bar**: a tab for each open session (with the host's OS icon), a **⋮** session menu and **+** for another connection. Below it is the first row of the **key panel**: the expand button, **Ask AI**, hidden input (**\*\*\***), <kbd>esc</kbd>, <kbd>tab</kbd>, <kbd>ctrl</kbd>, <kbd>alt</kbd>, a paper clip for files, the microphone, paste and the keyboard toggle. The first **Back** hides the keyboard; the second leaves the terminal (the session keeps running).

![Android terminal with the session bar](/docs/img/android-terminal.png)

Sessions in the bar are the ones of the current vault; two-finger swipe left or right on the terminal switches between them.

## Key panel

On a phone in portrait the compact bar keeps the scrolling key strip, the panel toggle and the keyboard button; **Ask AI**, hidden input, file attach, voice input and paste live under the **⋯** button so the keys themselves never shrink. On tablets and in landscape all of them sit inline.

Expand the panel for rows of extra keys: arrows and editing keys, navigation (Home, End, PgUp, PgDn), symbols, brackets, F1–F12 and a **tmux** row with the common <kbd>Ctrl</kbd>-<kbd>B</kbd> chords (new window, next, previous, detach, split, zoom, copy mode…). The expanded bar also holds **Snippets**, **History & themes**, **Ask AI** and **Compose**.

![Extra key rows](/docs/img/android-key-panel.png)

**Settings → Keyboard & gestures** lets you reorder, hide or add rows and change what the modifiers do (sticky <kbd>Ctrl</kbd>/<kbd>Alt</kbd> apply to the next key). **Customize** at the top of the expanded panel opens the same screen.

### Hidden input

**\*\*\*** opens a field whose contents are not shown on screen and not echoed; the text is sent to the terminal with <kbd>Enter</kbd>. Use it for passwords, tokens and anything else you do not want in a screen recording or over someone's shoulder.

![Hidden input](/docs/img/android-hidden-input.png)

## Gestures

| Gesture                               | Effect                                                                                                      |
| ------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Tap                                   | Focus; show the keyboard if it was hidden                                                                   |
| **Long-press, then drag**             | Arrow keys in the direction of the drag — ←/→ move the cursor, ↑/↓ walk the command history; hold to repeat |
| Double-tap                            | <kbd>Tab</kbd> (completion), or select the word when the option is off                                      |
| Swipe up/down with one finger         | Scroll the output                                                                                           |
| Pinch                                 | Text size                                                                                                   |
| Two-finger swipe left/right           | Previous / next session                                                                                     |
| Long-press with the arrow gesture off | Start a text selection; drag the handles, then copy, share or search                                        |

### The arrow gesture

Long-press anywhere on the output until the phone vibrates, then move the finger. A small indicator appears over the terminal: four arrows with the active direction lit and one to three dots for the current **gear**.

![Gesture indicator while dragging](/docs/img/android-gesture-overlay.png)

- Every cell the finger travels sends one arrow key; while the finger stays put the key repeats about seven times a second.
- Move further from the starting point for a higher gear: beyond five cells the repeat sends two keys per tick, beyond ten cells three. The dots show which gear you are in.
- **Cursor speed** (slow, normal, fast) in **Settings → Keyboard & gestures** scales how far the finger has to travel per cell: slow for precise edits in a long command, fast for walking back through history.
- Lift the finger and the indicator disappears. Nothing is sent before the long-press registers, so an ordinary scroll never moves the cursor.

![Gesture settings](/docs/img/android-gestures-settings.png)

Each gesture can be switched off individually: **Long-press moves the cursor**, **Double-tap sends Tab**, **Pinch to change text size**, **Two-finger swipe switches sessions**.

## Buttons and shake

**Volume up** / **Volume down** and **Shake the device** can be bound to an action in **Settings → Keyboard & gestures**: send a key — <kbd>Tab</kbd>, <kbd>Esc</kbd>, <kbd>Shift</kbd>+<kbd>Tab</kbd>, an arrow, any <kbd>Ctrl</kbd>/<kbd>Alt</kbd> chord or a literal string — or an app action: text size up/down, scroll a page, next/previous session, show/hide the keyboard, close the session, send a file, open Compose, voice input. Bound buttons act only while a terminal is on screen; elsewhere they change the volume as usual.

## Session menu

**⋮** in the session bar opens the menu for the current session: **Duplicate**, **Reconnect**, **Open SFTP** (a file browser on the same host, with the same credentials), **Port forwarding…**, **Terminal sharing**, **Copy address**, and below it the host's actions — edit, run a snippet on it, close all sessions to this host.

![Session menu](/docs/img/android-session-menu.png)

## Connections

The **Connections** tab shows every live thing in the app: terminal sessions as live previews of their screen, SFTP connections, and **active port-forwarding tunnels** with their rule and traffic. Tap a session to return to it, a tunnel to open its rule, or close any of them from the card. **Local terminal** and **Serial (USB)** start from here too.

![Connections](/docs/img/android-connections.png)

## Notification shade

While anything is connected, Termoso runs as a foreground service and shows a notification per connection: the host name, `user@host:port`, the OS icon, and a **Close** button (tunnels get **Stop**). Tapping a card opens exactly that session, file browser or forwarding screen. With several connections they collapse into one group with a summary — _2 terminals · 1 tunnel_ — and **Close all**.

![Sessions in the notification shade](/docs/img/android-shade.png)

The notification is silent and cannot be swiped away while a session is open; it disappears when the last connection closes. If a vendor's battery optimiser still kills background sessions, exclude Termoso from optimisation — see [Troubleshooting](/docs/reference/troubleshooting#android-specifics).

## Sharing text into the terminal

Share text from any app to Termoso and it is offered for pasting into the active session, with a preview first so nothing runs by accident. Shared images and files are uploaded to the host instead — see [Pasting images and files](/docs/terminal/compose-and-ai#pasting-images-and-files).

## Hardware keyboards

A Bluetooth or USB keyboard works as on the desktop: <kbd>Ctrl</kbd>, <kbd>Alt</kbd>, function keys and arrows go to the shell, <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>C</kbd>/<kbd>V</kbd> copy and paste. The key panel can be hidden while a keyboard is attached.
