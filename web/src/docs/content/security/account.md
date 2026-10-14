---
title: Account security
description: Password sign-in without sending the password, two-factor authentication, device approval and recovery.
---

## Sign-in with OPAQUE

Termoso signs you in with **OPAQUE**, a password-authenticated key exchange. The server stores a per-user record and checks a cryptographic proof; your password is never sent — not in plain text, not hashed. The same exchange produces the key that unwraps your account keys on the device.

## Two-factor authentication

Under **Security** in the cabinet or the app, add one or more second factors:

- authenticator app (TOTP);
- passkeys and security keys (WebAuthn / FIDO2), including from Android over USB or NFC;
- e-mail codes;
- backup codes for emergencies.

## New devices

A new device has to be approved from a device that is already signed in, or through the link the server e-mails you. The list of devices and their last activity is under **Devices**; revoking one signs it out immediately.

## Recovery key

At sign-up you get a 24-word recovery key. It is the only way back in if you forget your password — the server cannot reset it for you. Keep it offline.

## Single sign-on

Self-hosted servers can add OpenID Connect or SAML 2.0 providers. SSO proves who owns the e-mail address; your vault password and recovery key remain the only ways to unwrap key material, so the identity provider cannot read vaults.
