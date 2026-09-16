# Termoso for iOS: installing without the App Store

Termoso has no App Store listing and no paid Apple Developer account. Every
`v*` release ships two iOS assets:

| Asset | What it is |
|---|---|
| `termoso-<version>-ios.ipa` | the app, **unsigned**, built by GitHub Actions from the tagged commit (`.github/workflows/release.yml`, job `ios`) |
| `termoso-altstore.json` | an [AltStore](https://altstore.io) / [SideStore](https://sidestore.io) source that points at that `.ipa` (name, version, size, SHA-256, minimum iOS) |

An unsigned `.ipa` cannot be installed on a stock iPhone as-is: iOS only runs
code signed for *your* device. With a **free Apple ID** that signature is valid
for **7 days** and can only be produced on your side — CI never has your Apple
ID, device pairing, certificates or provisioning profiles, and the source JSON
contains none of them either. AltStore/SideStore do the signing and repeat it
before the week is over, so in practice the app just keeps working.

## Install (AltStore or SideStore)

1. Install AltStore (needs a Mac/PC with AltServer on the same Wi-Fi) or
   SideStore (no computer after the first pairing, uses an on-device VPN to
   talk to itself) following their own guides. Sign in with your Apple ID — the
   credentials go to Apple only, never to Termoso.
2. In the store app open **Sources → +** and add:

   ```
   https://github.com/rep0rtDev/termoso/releases/latest/download/termoso-altstore.json
   ```

   The URL never changes: `releases/latest/download/…` always resolves to the
   newest published release, and each release regenerates the JSON with its own
   `.ipa` URL and checksum.
3. Open the **Termoso** entry and tap **Install**. The store downloads the
   `.ipa`, signs it with your Apple ID and installs it.
4. First launch: **Settings → General → VPN & Device Management → trust** your
   Apple ID if iOS asks.

### Refresh ("7 days")

* A free Apple ID profile expires 7 days after signing. AltStore and SideStore
  refresh installed apps automatically in the background (Background App
  Refresh must be on; AltStore additionally needs AltServer reachable on the
  network at that moment). Opening the store app and tapping **Refresh All**
  does the same on demand.
* If the profile expires, the app icon stays but Termoso will not launch until
  you refresh — **nothing is lost**: the encrypted vault and the master key in
  the Keychain survive re-signing and updates.
* Free Apple IDs are limited to **3 sideloaded apps** and **10 app IDs per
  week**. A paid Developer account ($99/year) lifts both limits and signs for
  a year; the same `.ipa` works.

### Updates

A new release = a new `.ipa` + regenerated source. The store app shows the
update under **Updates** (or installs it automatically if you enabled that),
verifying the download against the `sha256` in the source. Termoso itself has
no updater on iOS and never phones home.

## Verify what you install

Every release also carries `SHA256SUMS.txt`:

```bash
sha256sum -c --ignore-missing SHA256SUMS.txt   # after downloading the .ipa next to it
jq '.apps[0].versions[0] | {version, downloadURL, sha256}' termoso-altstore.json
```

The `.ipa` is a plain zip: `unzip -l termoso-*-ios.ipa` shows one
`Payload/Termoso.app`. The Rust core (`crates/termoso-mobile`) is statically
linked into the app binary, so there are no embedded frameworks to inspect
separately.

## Other ways to install the same `.ipa`

* **Xcode** (free Apple ID): open `apps/ios/Termoso.xcodeproj` (after
  `./build-core.sh --device` and `xcodegen generate`), set your team under
  *Signing & Capabilities*, run on the connected iPhone. Same 7-day profile.
* **Sideloadly / iOS App Signer**: drag the `.ipa` in, sign with your Apple ID.
* **EU alternative marketplaces**: not applicable — those still require an
  Apple Developer account and notarization on the publisher's side.

## What CI does (and does not)

`release.yml` on a tag:

1. `apps/ios/build-core.sh --release --device --no-sim` — builds
   `termoso-mobile` for `aarch64-apple-ios`, generates the UniFFI Swift
   bindings and wraps the static library into `TermosoCoreFFI.xcframework`.
2. `xcodegen generate` → `xcodebuild archive` with code signing **disabled**.
3. Zips `Payload/Termoso.app` into `termoso-<version>-ios.ipa` and checks
   `CFBundleShortVersionString` equals the tag.
4. The `publish` job uploads the `.ipa`, renders the source with
   `apps/ios/altstore/render.py` (URL, size, SHA-256, build number read from
   the `.ipa`), uploads it as `termoso-altstore.json`, re-downloads every asset
   and verifies the checksum matches before the release goes public.

Nothing in the pipeline signs the app or needs Apple credentials. `ci.yml`
(job `ios`) builds the simulator slice on every PR and runs the XCTest and
XCUITest suites in an iPhone simulator, uploading screenshots and the
`.xcresult` as the `ios-simulator` artifact.

## Known limits of the iOS build

* This is the first iOS increment: encrypted local vault, hosts/groups/tags,
  settings, welcome flow (Termoso Cloud / self-hosted / offline). Terminal,
  SFTP, port forwarding, keychain, snippets, account sign-in and teams are
  placeholders and follow in the next milestones.
* iOS suspends background apps within seconds, so live sessions will only run
  while Termoso is on screen (same constraint as every iOS SSH client).
* iPhone only for now (`TARGETED_DEVICE_FAMILY = 1`); iPad comes later.
