---
title: Quick start
description: Install the app, add a host and connect — in about a minute, with or without an account.
---

## 1. Install

Grab the build for your platform from the [download page](/docs/getting-started/download) — `.deb`/`.rpm`/AppImage, Windows installer, macOS disk image, or the Android APK. The app starts in a **Local vault**: everything you save stays on this device until you decide to sign in.

## 2. Add a host

The Hosts screen has two ways in:

- **Quick connect** — type `user@host` or `user@host:port` into the search field and press <kbd>Enter</kbd>. The session opens in a new tab; the host is remembered in the connection history and can be saved from the tab menu later.
- **New host** — fill in the details panel: a name, the address, the user, and optionally tags, a group, notes and credentials. The address is the only required field; everything else has a default. The host is saved as you type.

![Host details on the desktop](/docs/img/desktop-host-details.png)

Coming from another client? **New host ▾ → Import…** reads `~/.ssh/config`, PuTTY sessions and Termius exports — see [Import existing hosts](/docs/getting-started/import).

## 3. Connect

Double-click the host (or select it and press **Connect**). The first connection to a server shows its host key fingerprint; **Trust & connect** pins the key in **Known Hosts**, and a different key for the same address is flagged on later connections.

![Host key confirmation](/docs/img/desktop-hostkey-prompt.png)

Whatever the server asks for next — a password, a key passphrase, a FIDO2 touch — is asked inside the connection card. Tick **Save to this host** and the password is stored encrypted in the vault, so the next connection goes straight to the prompt.

![Password prompt in the connection card](/docs/img/desktop-auth-prompt.png)

## 4. Work

![A connected terminal](/docs/img/desktop-terminal.png)

A few things worth knowing on day one:

- Right-click a tab for **Split right / down**, **Duplicate session**, **Open SFTP for this connection** and **Broadcast input** — see [Tabs, splits and workspaces](/docs/terminal/tabs-and-splits).
- <kbd>Ctrl</kbd>+<kbd>K</kbd> opens the command palette; <kbd>Ctrl</kbd>+<kbd>J</kbd> jumps to a host or tab. Every shortcut is listed under [Keyboard shortcuts](/docs/reference/keyboard-shortcuts).
- Autocomplete suggests commands from your own history and snippets as you type; <kbd>Tab</kbd> accepts.
- <kbd>Ctrl</kbd>+click opens a link in the output, including `ls --hyperlink` style OSC 8 links.

## 5. Optional: sign in

An account adds sync between devices and sharing with a team. Use the free cloud at `app.termoso.dev` or [your own server](/docs/self-hosting/overview) — the app asks for the server URL when you sign in. Hosts stay in the Local vault until you **Move** them to your personal vault; nothing is uploaded by signing in alone.

On the phone, sign in with the same account and the vault appears within seconds — see [Sync to mobile](/docs/getting-started/mobile).

> [!TIP]
> No account, no problem: the Local vault is a full vault. Keys, snippets, port forwarding, recordings and the Linux environment on Android all work offline. Sync is the only thing an account adds.
