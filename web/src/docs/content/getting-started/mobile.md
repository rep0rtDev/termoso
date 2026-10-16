---
title: Sync to mobile
description: Use the same hosts, keys and snippets on your phone by signing in to the same account — and what the phone keeps, locks and does on its own.
---

## Sign in on the phone

1. Install the [Android app](/docs/getting-started/download#android).
2. Open the vault screen, choose **Sign in** and enter the same server — `app.termoso.dev` for the free cloud or your own URL.
3. Enter your email and password. If the account has two-factor authentication, confirm with your code, security key (USB or NFC) or passkey.
4. Approve the new device from a device that is already signed in (**Settings → Account → Devices**), or from the e-mail the server sends.

Your personal vault and every team vault you have access to appear in the vault list within a few seconds, hosts first, then keys and snippets.

![Vaults on Android](/docs/img/android-vault.png)

> [!TIP]
> The password never leaves the phone either: sign-in uses the OPAQUE protocol, so the server only checks a proof. See [Account security](/docs/security/account).

## What the phone keeps

Vaults are stored encrypted on the phone; the database key is wrapped by the Android Keystore, which on most phones means hardware-backed. Turn on **Settings → Security → App lock** to require biometrics, the device credential or an app PIN/pattern before the vaults open, and **Lock after** to lock again in the background.

![Settings on Android](/docs/img/android-settings.png)

Per-device settings — appearance, key panel rows, gestures, volume buttons, recording, the Linux environment — are under **Settings** and do not sync; the vault contents do.

## Hosts that are only on the phone

The phone has its own Local vault too. Hosts you save there stay on the phone; long-press → **Move to** your personal vault to see them on the desktop. Quick connects from the search field are not saved anywhere until you save them.

## Working offline

Everything in a synced vault is on the phone, so hosts, keys and snippets are available without a network, and changes made offline (a new host, an edited note) are pushed when the phone is back online. The Linux environment, serial consoles and local recordings never needed a network in the first place.

## Signing out

**Settings → Account → Sign out** removes the synced vaults from the phone and keeps the Local vault. **Remove this device** from another device does the same remotely, and also revokes the phone's session on the server — the first thing to do when a phone is lost, together with changing the account password.
