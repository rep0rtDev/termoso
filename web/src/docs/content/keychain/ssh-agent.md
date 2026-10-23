---
title: SSH agent
description: Let ssh, git and your editor use keys from the vault without the private keys ever touching the disk.
---

Termoso can act as the **SSH agent** of your computer. Turn it on in *Settings → SSH agent → Serve vault keys as an SSH agent*; from then on local tools that speak the agent protocol — OpenSSH `ssh`, `scp`, `git`, VS Code, JetBrains IDEs, Ansible — can authenticate with the private keys stored in your vault. The keys are decrypted only inside Termoso; a program asking for a signature gets the signature back, never the key.

## Pointing tools at it

The settings card shows the line for your platform. Put it in your shell profile (`~/.zshrc`, `~/.bashrc`, PowerShell `$PROFILE`):

```sh
export SSH_AUTH_SOCK="$HOME/.local/share/termoso/agent.sock"   # Linux (path shown in Settings)
```

```powershell
$env:SSH_AUTH_SOCK = '\\.\pipe\termoso-ssh-agent'                # Windows
```

Then `ssh-add -L` lists the keys Termoso serves, and `ssh user@host` uses them like any agent key. The socket is created with mode `0600`; on Windows the named pipe is reachable only by your user.

## Which keys are served

Every private key in an unlocked vault that Termoso can decrypt:

- plain keys, and encrypted keys whose passphrase is **remembered** in the keychain (*key → Change passphrase → Remember*);
- **not** FIDO2 / `sk-*` keys — the token signs only inside Termoso's own connections;
- **not** keys imported *from* another agent (there is no private half in the vault).

Keys you cannot use through the agent are listed under the toggle with the reason. The list follows the keychain: add, import or delete a key and the agent picks it up.

## Confirming signatures

With **Confirm every signature** (on by default) each request pops a prompt: which program asked (name and pid on Linux), which key, its fingerprint — *Allow* or *Deny*. Turn it off to sign silently; every request is still written to the application log.

## When it stops

Locking the vault (manually or after the idle timeout) and quitting Termoso stop the agent and forget the decrypted keys; unlocking starts it again. The agent accepts only *list* and *sign* requests — `ssh-add` cannot add, remove or lock keys in Termoso, so nothing on the machine can inject a key into your vault through it.

## Compared with the system agent

*Use the system SSH agent*, the toggle above this one, is the other direction: Termoso **uses** keys held by `ssh-agent`, the Windows OpenSSH agent or Pageant when it connects. Both can be on at the same time, but do not point `SSH_AUTH_SOCK` at Termoso while that is enabled — Termoso would then be asking itself.
