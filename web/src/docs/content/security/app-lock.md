---
title: App lock
description: Require a master password, biometrics or a PIN before Termoso opens your vaults on a device — and what happens to keys in memory when it locks.
---

## Desktop: master password

Set a master password in **Settings → Security**. From then on **App Lock** is always on: Termoso asks for the password on start before it opens hosts, keys and snippets, and the local database key is wrapped with a key derived from it (Argon2id).

![Security settings](/docs/img/desktop-settings-security.png)

| Setting                                                      | Effect                                                                        |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------- |
| **Lock on start**                                            | Ask for the password every time the app starts.                               |
| **Lock after inactivity**                                    | Lock again after the chosen idle time (5, 15, 30 minutes and longer).         |
| **Lock now** / <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>L</kbd> | Lock immediately, for example before you walk away.                           |
| **Change master password**                                   | Re-encrypts the local key; your vault data does not have to be re-downloaded. |

While locked, open terminal sessions keep running (a long `rsync` does not care) but their contents are hidden, and nothing in the vault can be read until the password is entered. Forgetting the master password means re-downloading synced vaults after signing in again; the Local vault on that device is lost, so write it down.

## Android

**Settings → Security** offers:

- **App lock** with biometrics or the device credential, through the system prompt;
- **App PIN / pattern** — a 6-digit PIN or a pattern asked for when Termoso is locked, in addition to the screen lock. It is stored only on the device, as a salted hash, and locks out for increasing intervals after repeated failures;
- **Lock after** — immediately or after a delay in the background;
- **Remember key passphrases in memory** — keep passphrases typed at connect time until the app is locked.

![App PIN on Android](/docs/img/android-pin.png)

While a lock is on, Termoso blocks screenshots and blurs its window in the recent-apps list, and the notification shade shows only that sessions are active, not their names.

## What locking protects

The lock guards the keys in memory and the UI. Data at rest is always encrypted — on Android with a key in the Keystore, on the desktop with the master password if one is set, otherwise with a key stored in the operating system's credential store (Secret Service, Keychain, Credential Manager). Someone with your unlocked phone or laptop and no app lock can read your hosts; someone with the disk alone cannot.
