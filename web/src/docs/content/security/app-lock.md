---
title: App lock
description: Require a master password, biometrics or a PIN before Termoso opens your vaults on a device.
---

## Desktop: master password

Set a master password in **Settings → Security**. From then on **App Lock** is always on: Termoso asks for the password on start before it opens hosts, keys and snippets.

| Setting                   | Effect                                              |
| ------------------------- | --------------------------------------------------- |
| **Lock on start**         | Ask for the password every time the app starts.     |
| **Lock after inactivity** | Lock again after the chosen idle time.              |
| **Lock now**              | Lock immediately, for example before you walk away. |

**Change master password** re-encrypts the local key; your vault data does not have to be re-downloaded.

## Android

**Settings → Security** offers:

- **App lock** with biometrics or the device credential;
- **App PIN / pattern** — a 6-digit PIN or a pattern asked for when Termoso is locked, in addition to the screen lock. It is stored only on the device, as a salted hash, and locks out after repeated failures;
- **Lock after** — immediately or after a delay in the background;
- **Remember key passphrases in memory** — keep passphrases typed at connect time until the app is locked.

While a lock is on, Termoso blocks screenshots and blurs its window in the recent-apps list.

![App PIN on Android](/docs/img/android-pin.png)
