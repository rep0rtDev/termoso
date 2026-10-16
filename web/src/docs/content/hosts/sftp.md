---
title: Files: SFTP and WebDAV
description: A two-pane file manager for SFTP and WebDAV hosts, with a transfer queue, server-to-server copies, editing in place and system Files integration on Android.
---

Open **SFTP** in the sidebar, then pick a host or an already connected session for the right pane. The left pane is your local machine by default; both panes can be remote, and a tab can hold a different pair of panes than the next.

![Two-pane SFTP](/docs/img/desktop-sftp.png)

A terminal session can open a file browser on the same connection from its tab menu — **Open SFTP for this connection** — without authenticating again.

## Browsing

- Breadcrumbs at the top of each pane jump to any parent; **←** / **→** walk the pane's history; typing a path into the breadcrumb goes there directly.
- Sort by name, date, size or kind; **Filter** narrows the list as you type. Hidden files are toggled per pane.
- The permission string and owner are shown under each name; **Edit Permissions** changes mode bits with checkboxes or an octal value.
- Double-click a folder to enter it, a file to open it with the default application; **Open with…** picks another one.

## Transfers

Drag files or folders between panes, or from your desktop into a pane, or use **Upload** / **Download** in the **Actions** menu. Every transfer goes into a **queue** with progress per file and overall, pause/resume, and a conflict prompt — **Replace**, **Keep both**, **Skip**, with _apply to all_.

![A transfer in progress](/docs/img/desktop-sftp-transfer.png)

Folders are transferred recursively; symlinks are copied as the files they point to. Transfers continue while you browse elsewhere in the app and are listed in the queue until you clear them.

### Between two servers

With a remote host in both panes, drag from one to the other — or **Copy to target directory** — and the file is streamed through the app from one server to the other. It works even when the two servers cannot reach each other, and nothing is written to the local disk on the way.

## Editing in place

**Edit** opens a remote file in your editor (the system default, or the one set in **Settings → Files**). When you save, the file is uploaded back; the pane shows a small indicator while a file is open for editing. Useful for config files; for anything larger than a few megabytes download first.

## WebDAV

A host can carry a WebDAV endpoint next to (or instead of) SSH: **Add WebDAV** in the host details, with the URL and Basic or Digest credentials. The browser is the same. A self-signed certificate can be pinned on first use instead of being rejected, and the pin is stored with the host in the vault.

## Android

Every SFTP and WebDAV host is exposed to the system **Files** app and to document pickers (Storage Access Framework) when **Settings → Files → Show SFTP hosts in Files** is on, so other apps can open and save files on your servers directly — attach a log to an e-mail, open a config in a code editor, save a photo to a server. Hosts in a locked vault are hidden until the vault is unlocked.

The in-app browser has the same transfer queue with pause and resume, **Copy to…** between hosts, and uploads from the phone's storage or the camera. Open it from a session's **⋮ → Open SFTP**, from **Connections**, or from the host's menu.

![Files on Android](/docs/img/android-sftp.png)

## Connecting with SFTP credentials

**Connect via SFTP** on a host reuses the SSH credentials of the host, including keys, certificates and security keys; a password typed for the terminal is offered for the file session in the same card.

> [!NOTE]
> SFTP runs over the SSH connection; there is nothing to enable on the server beyond `Subsystem sftp` in `sshd_config`, which every distribution ships with. Servers that only offer SCP are not supported.
