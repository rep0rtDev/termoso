---
title: Download and install
description: Native builds for every platform, published with each release on GitHub. No store account, no installer telemetry.
---

Every release on [GitHub](https://github.com/rep0rtDev/termoso/releases/latest) ships the same version for all platforms, together with `SHA256SUMS.txt` and signatures for the desktop updater.

## Desktop

| Platform               | Files                                              | Notes                                                                                                        |
| ---------------------- | -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| Linux x86_64 / aarch64 | `.deb`, `.rpm`, `.AppImage`                        | The AppImage runs on any recent distribution without installing.                                             |
| Windows x86_64         | `_x64-setup.exe`, `_x64_en-US.msi`                 | The `.exe` installs per user, the `.msi` suits managed deployments.                                          |
| macOS                  | `_aarch64.dmg` (Apple Silicon), `_x64.dmg` (Intel) | Ad-hoc signed: on first launch right-click the app → **Open**, or run `xattr -cr /Applications/Termoso.app`. |

### Updates

Nothing is fetched unless you ask. **Settings → Updates** has two modes — **Only when I ask** (default) and **Once at startup** — and a **Release feed** field that defaults to GitHub and can point at your own server. Every update is verified against the project's signing key before it is installed.

## Android

APKs are signed with the project key and need no Google Play Services:

- `termoso-<version>-arm64-v8a.apk` — almost every phone made since 2017;
- `termoso-<version>-armeabi-v7a.apk` — older 32-bit devices;
- `termoso-<version>-x86_64.apk` — emulators and Chromebooks;
- `termoso-<version>-universal.apk` — all of the above in one larger file.

Allow installing from your browser or file manager when Android asks. There is no in-app updater: install a newer APK over the old one, your data stays.

## iOS

There is no App Store listing. Each release has an unsigned `termoso-<version>-ios.ipa` and an AltStore/SideStore source, `termoso-altstore.json`. Add the source in AltStore or SideStore; the store app signs Termoso with your own Apple ID and refreshes it every seven days.

> [!NOTE]
> The iOS app is younger than the others: it has the encrypted local vault, the host editor and the SSH terminal, but not yet account sync, SFTP or port forwarding.

## Verify a download

```bash
sha256sum --ignore-missing -c SHA256SUMS.txt
```

The checksum file is produced by the same release workflow that builds the binaries, from the public repository.
