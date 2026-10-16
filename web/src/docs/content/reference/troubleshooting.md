---
title: Troubleshooting
description: The errors people actually hit — connection refused, host key changed, permission denied, tunnels that will not bind, sync that does not arrive — and what to do about each.
---

Each section starts with the message you see, then the usual causes in order of likelihood.

## Connecting

### "Connection refused" or "timed out"

- The address or port is wrong, or the server is down. Try the same thing from a shell: `nc -vz host 22`.
- A firewall between you and the server. Timeouts (not refusals) usually mean a firewall drops packets silently; refusals mean something answered and said no.
- The host is only reachable through a bastion: set a **Jump host** in the host details instead of connecting directly.
- IPv6 is advertised but not routed: set **IP version → IPv4** for that host.

### "Host key has changed"

Termoso stops before sending anything and shows both fingerprints. If the server was reinstalled or its keys rotated, open **Known Hosts**, remove the old entry for that address and connect again. If you did not expect a change, do not connect until you can verify the new fingerprint through another channel — this is the one warning that is worth a phone call.

![Host key confirmation](/docs/img/desktop-hostkey-prompt.png)

### "Permission denied (publickey)"

- The key in the host's credentials is not in `~/.ssh/authorized_keys` on the server. **Keychain → key → Export to host…** appends it using credentials that still work (a password, another key).
- The server only accepts certificates or a specific key type. Check with `ssh -v` what it offers; `PubkeyAcceptedAlgorithms` on old servers may exclude Ed25519 or RSA-SHA2.
- The key is in your system agent and the agent is not running, or the public-only key in the Keychain no longer matches the agent's. `ssh-add -L` shows what the agent holds.
- Wrong username: groups pass their username down, and a host with its own blank username inherits it.

### "Permission denied" with a password that is correct

Password authentication is disabled on the server (`PasswordAuthentication no`), or the server uses keyboard-interactive and PAM rejects the login for another reason (expired password, locked account). The connection card shows which method is being tried.

### Authentication prompt closed by itself

On Android a tap outside the credentials sheet cancels the whole connection. Use **Cancel** or the back gesture deliberately; otherwise answer the prompt and press <kbd>Enter</kbd>.

### Mosh does not connect

Mosh needs `mosh-server` on the host and UDP 60000–61000 open towards it. The SSH leg works first (that is how Mosh starts), then the UDP session; a firewall that only allows TCP produces exactly "SSH works, Mosh hangs".

## Terminal

### Characters are garbled or boxes appear

- The server's locale is not UTF-8: `export LANG=C.UTF-8` on the host, or set **Character set** in the host's terminal settings for legacy equipment.
- Icons in a prompt render as boxes: the prompt uses code points outside the Nerd Fonts ranges; see [Fonts](/docs/terminal/appearance#nerd-font-glyphs).
- Serial consoles that show rubbish: wrong baud rate, or the device uses 7E1 instead of 8N1.

### Backspace or Delete does the wrong thing

Switch **Backspace sends** between `^?` and `^H` in the host's terminal settings — different embedded systems and old Unixes disagree. Delete problems are usually `TERM`: Termoso advertises `xterm-256color`; set the host's terminal type to `linux` or `vt100` for equipment that does not know it.

### Colours are wrong in `tmux` or `vim`

Set `set -g default-terminal "tmux-256color"` (tmux) or `set termguicolors` (vim) on the host. If a TUI insists on 16 colours, check that `TERM` was not overridden by a profile script.

### A shortcut reaches Termoso instead of the program

Unbind it in **Settings → Keyboard**; unbound chords are passed through to the shell. See [Keyboard shortcuts](/docs/reference/keyboard-shortcuts#inside-the-terminal).

### Autocomplete shows nothing

Suggestions come from history, snippets and — if enabled — the remote shell history. A fresh host has none yet. Prompt markers (OSC 133) make suggestions precise; without them Termoso guesses the prompt line, which fails on exotic prompts. The popup can be turned off entirely in **Settings → Terminal**.

### The terminal on Android ignores my taps

Long-press is reserved for the arrow gesture when **Keyboard & gestures → Long-press moves the cursor** is on; a single tap places focus and brings up the keyboard. Selection starts with a long-press when the arrow gesture is off, or from the selection action in the session menu.

## SFTP

### "Permission denied" when uploading

The target directory is owned by another user. Upload to your home directory and move the file with `sudo mv` from the terminal, or **Edit Permissions** on the directory if it is yours.

### Transfers are slow

Termoso pipelines SFTP requests, so throughput should be close to `scp`. If it is far below, the bottleneck is usually the server's disk or a VPN MTU problem; a quick comparison is `scp` of the same file from the same machine.

### The host does not appear in the Android Files app

Turn on **Settings → Files → Show SFTP hosts in Files**, then pull to refresh in the Files app. Hosts in a locked vault are hidden until the vault is unlocked.

## Port forwarding

### "Address already in use"

Something on this machine already listens on the local port: `ss -ltnp | grep :8080` (Linux), `lsof -i :8080` (macOS), `netstat -ano | findstr :8080` (Windows). Pick another local port or stop the other program.

### "Permission denied" binding a port below 1024

Ports under 1024 need administrator rights on most systems. Use `8080 → 80` instead of `80 → 80`, or run `setcap` tricks at your own risk.

### The tunnel starts but nothing answers

The **destination** is resolved on the **server**, not on your machine: `localhost:5432` means the database on the server itself. If the service runs on a third machine, put that machine's address as the destination and make sure the server can reach it (`nc -vz 10.0.2.5 5432` from the server). The rule's status line shows the first error the server returned.

### Remote forwarding does not work

`GatewayPorts` and `AllowTcpForwarding` must permit it in `sshd_config` on the server; many hardened servers set `AllowTcpForwarding no`.

## Sync and account

### Changes do not appear on the other device

- Check the vault: items in the **Local vault** never sync. Move them to your personal vault with **Move to**.
- The device is not approved yet. **Settings → Account → Devices** on a signed-in device lists pending approvals.
- The other device is offline or asleep; changes arrive when it reconnects.
- A team vault shows **Locked on this device**: a manager has to hand you the key by opening the app once after a key rotation.

### "Server certificate not trusted" on a self-hosted server

The apps ship their own root store and do not use the operating system's. Private CAs are not accepted — use a Let's Encrypt or other publicly trusted certificate on `TERMOSO_PUBLIC_URL`.

### Passkeys do not work with the desktop app

`TERMOSO_WEBAUTHN__RP_ID` must be the domain of `TERMOSO_PUBLIC_URL`, and the allowed origins must include `tauri://localhost,http://tauri.localhost`. See [Configuration](/docs/self-hosting/configuration#passkeys-app-links-sso).

### I forgot my password

Use the 24-word recovery key from sign-up. Without it there is no way back in — not for you and not for the server operator, because the server never had the key. The Local vault on each device remains readable with the device's own protection (master password, biometrics).

## Android specifics

### The session died while the app was in the background

Android stopped the foreground service — usually a battery optimiser. Exclude Termoso from battery optimisation (**Android Settings → Apps → Termoso → Battery → Unrestricted**). Some vendors (Xiaomi, Huawei, OnePlus) have a second switch for autostart or background activity.

### Linux environment: "can't fork" on Alpine

Only on x86_64 devices and emulators; musl's `fork()` uses a system call that Android blocks. Use Ubuntu there — on arm64 phones Alpine works.

### `apt` cannot resolve hosts in the Linux environment

The resolver file is rewritten from the phone's DNS at every shell start. If a VPN changed the DNS while a shell was open, close and reopen the Linux tab.

## Getting help

Issues and discussions live on [GitHub](https://github.com/rep0rtDev/termoso/issues). Include the app version (**Settings → About**), the platform, and — for connection problems — the output of `ssh -vv user@host` from the same network. Never paste keys, passwords or recovery words; Termoso's own logs do not contain them, and neither should an issue.
