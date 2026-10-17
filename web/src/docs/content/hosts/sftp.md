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

A host can carry a WebDAV share next to (or instead of) SSH: **Add WebDAV** in the host details. The share lives at the host's **address** — the same one SSH uses — so the section only asks for what differs from SSH:

| Field | Meaning |
| --- | --- |
| **Port** | TCP port; empty means 443, or 80 when plain HTTP is on. |
| **Path** | The collection below the server root, e.g. `/remote.php/dav/files/alice/` for Nextcloud or `/dav/` for a bare server. Empty means `/`. |
| **Plain HTTP** | Talk HTTP without TLS. Only for a share on the local network or behind a tunnel — credentials travel in the clear. |
| **Authentication** | Basic/Digest (user + password) or a bearer token; both are stored in the vault, never in the host record. |

The browser is the same as for SFTP, and WebDAV hosts show up in the same places (Connections, **Copy to…**, the Android Files app).

### TLS

The **TLS** block covers everything about trusting the server; it is hidden for plain-HTTP shares.

- **Server name (SNI)** — only when the certificate is issued to a different name than the address: a share behind a reverse proxy that you reach by IP, or an internal name that is not in DNS. Termoso connects to the address, but sends this name as SNI and `Host` and checks the certificate against it.
- **Certificate authority (PEM)** — your own CA (one or several certificates, PEM) that signed the server certificate. With a CA set, the system trust store is not consulted; the server chain must lead to one of the pasted certificates.
- **Server certificate fingerprint** — pins exactly one certificate by SHA-256 (`sha256:…`, bare or colon-separated hex). A pin wins over the CA and over the system roots.
- **Client certificate** — a certificate + private key pair for mutual TLS, stored in the vault. See [Keychain](/docs/keychain/identities) for identities.

With neither CA nor pin, the system trust store decides. An unknown certificate is not rejected outright: the first connection shows its fingerprint and offers to pin it, and the pin is stored with the host.

> [!NOTE]
> Hosts created before 0.8.2 carried a full WebDAV URL. They keep working as they are; opening such a host in the editor shows the URL split into address, port, path and HTTP, and saving stores the new form.

## Android

Every SFTP and WebDAV host is exposed to the system **Files** app and to document pickers (Storage Access Framework) when **Settings → Files → Show SFTP hosts in Files** is on, so other apps can open and save files on your servers directly — attach a log to an e-mail, open a config in a code editor, save a photo to a server. Hosts in a locked vault are hidden until the vault is unlocked. Connections that another app opens this way run quietly: they do not appear as cards in the notification shade and close on their own after a few minutes idle; only sessions you open in Termoso itself are listed there.

The in-app browser has the same transfer queue with pause and resume, **Copy to…** between hosts, and uploads from the phone's storage or the camera. Open it from a session's **⋮ → Open SFTP**, from **Connections**, or from the host's menu.

![Files on Android](/docs/img/android-sftp.png)

## Connecting with SFTP credentials

**Connect via SFTP** on a host reuses the SSH credentials of the host, including keys, certificates and security keys; a password typed for the terminal is offered for the file session in the same card.

> [!NOTE]
> SFTP runs over the SSH connection; there is nothing to enable on the server beyond `Subsystem sftp` in `sshd_config`, which every distribution ships with. Servers that only offer SCP are not supported.
