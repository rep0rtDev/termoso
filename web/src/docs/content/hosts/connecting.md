---
title: Connecting to a server
description: Saved hosts, quick connect, authentication prompts, host keys and the connection options a host can carry.
---

## Quick connect

Type into the search field on the Hosts screen and press <kbd>Enter</kbd>:

```
deploy@10.0.1.12
deploy@10.0.1.12:2222
ssh://deploy@edge.example.io
telnet://switch.lab:23
```

The session opens in a new tab.

## Saved hosts

**New host** opens the details panel. The address is all that is required; everything else has a sensible default and is saved as you type.

![Host details on the desktop](/docs/img/desktop-host-details.png)

| Field                     | Notes                                                                                                                        |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Address, port, IP version | Hostname or IP; `Auto`, `IPv4` or `IPv6`.                                                                                    |
| Protocol                  | SSH (default), Mosh, Telnet, local shell. Add **Telnet** or **WebDAV** to the same host with the buttons at the bottom.      |
| Credentials               | Username plus a password, a key or identity from the Keychain, an SSH certificate, a FIDO2 security key or an SSH ID handle. |
| Group, tags, notes        | Groups pass their settings to hosts without their own.                                                                       |
| Jump host                 | Another saved host to hop through (`ProxyJump`). Chains of several hops work.                                                |
| Proxy                     | HTTP, SOCKS4 or SOCKS5.                                                                                                      |
| Terminal                  | Color scheme, character set, backspace behaviour, startup snippet, environment variables.                                    |
| Agent forwarding, X11     | Off by default; X11 needs an X server on the local machine.                                                                  |

## Authentication prompts

Whatever the server asks for — password, key passphrase, keyboard-interactive questions, a FIDO2 touch, a PIN — appears **inside the connection card**, in the same place, while the progress line shows which step you are on. On Android the same card is a bottom sheet above the keyboard.

![Password prompt inside the connection card](/docs/img/desktop-auth-prompt.png)

An empty keyboard-interactive round (some servers send one) is answered automatically instead of showing an empty dialog.

## Host keys

The first connection shows the server's key fingerprint. Confirming it stores the key in **Known hosts** for that vault; from then on a different key for the same address stops the connection and shows the new fingerprint before anything is sent. Known hosts sync with the vault, so a key you confirmed on the desktop is trusted on your phone.

## Post-quantum key exchange

When the server supports it, the SSH key exchange uses the ML-KEM hybrid (`mlkem768x25519-sha256`) and the tab shows a small **PQ** badge. Nothing to configure; legacy servers keep working.

## Reconnecting

A dropped SSH session offers **Reconnect** in the terminal. Mosh sessions survive network changes and sleep on their own.
