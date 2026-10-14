---
title: FIDO2 security keys
description: SSH keys that live on a YubiKey or another FIDO2 authenticator, used from the desktop and from Android over USB or NFC.
---

FIDO2 SSH keys (`sk-ssh-ed25519@openssh.com`, `sk-ecdsa-sha2-nistp256@openssh.com`) keep the private key on the hardware token. Every login needs the token to be present and touched.

## Creating one

**Keychain → Generate FIDO2 Key** creates a key on the token. Choose whether a touch is required (default) and whether the token PIN must be entered. Resident (discoverable) keys can be read back from the token on another device.

## Using it

Pick the key in the host's credentials like any other. During authentication the connection card asks you to touch the token, or for the PIN.

## Android

Plug the token in over USB-C (or an OTG adapter), or hold an NFC token to the back of the phone when the card asks for it. The same keys work for signing in to your account as a second factor.
