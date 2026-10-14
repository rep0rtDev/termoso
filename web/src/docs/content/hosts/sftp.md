---
title: Files: SFTP and WebDAV
description: A two-pane file manager for SFTP and WebDAV hosts, with a transfer queue, editing in place and system Files integration on Android.
---

Open **SFTP** in the title bar, then pick a host or an already connected session in the right pane. The left pane is your local machine; both panes can be remote.

![Two-pane SFTP](/docs/img/desktop-sftp.png)

## What you can do

- Browse, with hidden files toggled per pane and sorting by name, size, date or permissions; the permission string is shown under each file name.
- Drag and drop between panes or from your desktop, or use **Upload** / **Download**. Transfers go into a **queue** with progress, pause/resume and a conflict prompt (replace, keep both, skip).
- **Edit in place**: open a remote file in your editor; changes are uploaded back when you save.
- Rename, delete, create folders, **Copy path**, **Edit Permissions**, **Open with…** another application.
- With a remote host in both panes, **Copy to target directory** copies between two servers through the app, so it works even when they cannot reach each other.

## WebDAV

A host can carry a WebDAV endpoint next to (or instead of) SSH: **Add WebDAV** in the host details. Basic and Digest authentication are supported, and a self-signed certificate can be pinned on first use instead of being rejected.

## Android

Every SFTP and WebDAV host is exposed to the system **Files** app and to document pickers (Storage Access Framework) when **Show SFTP hosts in Files** is on, so other apps can open and save files on your servers directly. The in-app file browser has the same transfer queue with pause and resume, and **Copy to…** between hosts.

![Files on Android](/docs/img/android-sftp.png)

## Connecting with SFTP credentials

**Connect via SFTP** on a host reuses the SSH credentials of the host, including keys, certificates and security keys; a password typed for the terminal is offered for the file session in the same card.
