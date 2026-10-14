---
title: Sync to mobile
description: Use the same hosts, keys and snippets on your phone by signing in to the same account.
---

## Sign in on the phone

1. Install the [Android app](/docs/getting-started/download#android).
2. Choose **Sign in** and enter the same server — `app.termoso.dev` for the free cloud or your own URL.
3. Enter your email and password. If the account has two-factor authentication, confirm with your code, security key or passkey.
4. Approve the new device from a device that is already signed in (or from the e-mail the server sends).

Your personal vault and every team vault you have access to appear in the vault list within a few seconds.

> [!TIP]
> The password never leaves the phone either: sign-in uses the OPAQUE protocol, so the server only checks a proof. See [Account security](/docs/security/account).

## What the phone keeps

Vaults are stored encrypted on the phone; the key is wrapped by the Android Keystore. Turn on **Settings → Security → App lock** to require biometrics, the device credential or an app PIN/pattern before the vaults open.

## Hosts that are only on the phone

The phone has its own Local vault too. Hosts you save there stay on the phone; move them to your personal vault to see them on the desktop.
