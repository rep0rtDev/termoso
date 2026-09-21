# Building Termoso from source

This is the from-scratch guide: a clean machine, a `git clone`, and at the end
the same artifacts the release workflow publishes — the server and bridge
container images, the web cabinet, desktop installers for Linux, Windows and
macOS, Android APKs and the iOS `.ipa`. Nothing here depends on GitHub
Actions or on any credential of the project; where the official builds are
signed, you sign with your own keys (or skip signing where that is allowed).

For *developing* (hot reload, running tests) see the shorter
[Local development](../README.md#local-development) section of the README and
[CONTRIBUTING.md](../CONTRIBUTING.md). For the automated release pipeline see
[RELEASING.md](RELEASING.md).

## Contents

- [Before you start](#before-you-start)
- [Toolchains](#toolchains)
- [Server and bridge (container images)](#server-and-bridge-container-images)
- [Server and bridge (native binaries)](#server-and-bridge-native-binaries)
- [Web cabinet](#web-cabinet)
- [Desktop](#desktop)
  - [Linux](#linux)
  - [Windows](#windows)
  - [macOS](#macos)
  - [Update signing key](#update-signing-key)
- [Android](#android)
- [iOS](#ios)
- [Verifying a build](#verifying-a-build)
- [Reproducibility notes](#reproducibility-notes)
- [Troubleshooting](#troubleshooting)

## Before you start

```bash
git clone https://github.com/rep0rtDev/termoso.git
cd termoso
git checkout v0.3.0          # a release tag; omit to build `main`
```

Everything below is run from the repository root unless a `cd` says
otherwise. Paths in the output examples use the version `0.3.0`; yours will
show whatever the workspace `Cargo.toml` says.

**One version for everything.** The workspace `Cargo.toml` `version` is the
source of truth. Android reads it at build time; `apps/desktop/package.json`
and `apps/ios/project.yml` (`MARKETING_VERSION`) carry the same number and the
release workflow refuses to run when they differ. If you fork and bump, change
all three.

**No cross-compiling between desktop OSes.** Tauri builds an installer for the
operating system it runs on: Linux packages on Linux, `.msi`/`.exe` on
Windows, `.dmg` on macOS (both CPU architectures from one Mac). Container
images, the web cabinet, the Android APKs and the server binaries build on any
Linux (Android also on macOS/Windows with the same SDK). iOS needs a Mac.

**Network access during the build.** `cargo`, `npm` and Gradle download
dependencies pinned in `Cargo.lock`, `package-lock.json` and
`apps/android/gradle/libs.versions.toml`. In addition the Tauri bundler fetches
`linuxdeploy` and its plugins for the AppImage on Linux, NSIS and WiX on
Windows, and the WebView2 bootstrapper is downloaded by the installer at
install time (not at build time).

## Toolchains

| Component | Needs |
|---|---|
| All Rust code | Rust **stable, 1.94 or newer** via [rustup](https://rustup.rs) (`rust-version` in `Cargo.toml` is the floor; CI uses the current stable). |
| Server / bridge images | Docker 24+ with BuildKit (default), or Podman 4.4+ — nothing else, the Dockerfiles bring their own toolchains. |
| Server / bridge binaries | Rust; on Linux `pkg-config` and `libssl-dev` (Debian/Ubuntu) or `openssl-devel` (Fedora). |
| Web cabinet | Node **22.12+** with npm, `wasm-pack` 0.13+ (`cargo install wasm-pack` or the [installer](https://github.com/wasm-bindgen/wasm-pack/releases)), Rust target `wasm32-unknown-unknown` (`rustup target add wasm32-unknown-unknown`). |
| Desktop | Node 22.12+, Rust, and the [Tauri 2 prerequisites](https://tauri.app/start/prerequisites/) for your OS (listed per OS below). |
| Android | JDK **17**, Android SDK with **platform 36**, **build-tools 36.0.0**, **NDK 27.2.12479018**; `cargo-ndk` 4.x (`cargo install cargo-ndk`); Rust targets `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`. |
| iOS | macOS with **Xcode 16**, `xcodegen` (`brew install xcodegen`), Rust targets `aarch64-apple-ios` and `aarch64-apple-ios-sim` (the build script adds missing ones). |

Check what you have:

```bash
rustc --version && cargo --version      # ≥ 1.94
node --version && npm --version         # ≥ 22.12
wasm-pack --version                     # web cabinet only
docker --version                        # or: podman --version
java -version                           # 17.x, Android only
cargo ndk --version                     # Android only
```

## Server and bridge (container images)

The Dockerfiles are self-contained multi-stage builds: they compile the WASM
crypto module, the web cabinet and the Rust server inside the build and copy
only the results into a distroless runtime image that runs as `nonroot`. No
host toolchain other than Docker/Podman is needed, and the build context is the
repository root.

```bash
# API server + web cabinet in one image (what ghcr.io/rep0rtdev/termoso-server is)
docker build -f deploy/Dockerfile -t termoso-server:local .

# API Bridge (what ghcr.io/rep0rtdev/termoso-bridge is)
docker build -f deploy/Dockerfile.bridge -t termoso-bridge:local .
```

With Podman replace `docker build` by `podman build` (BuildKit cache mounts are
supported since Podman 4.x). A cold build compiles the whole dependency tree
and takes 10–25 minutes; the `--mount=type=cache` layers make rebuilds
incremental.

Smoke test the server image without any database — it refuses to start
without a master key, which proves the binary runs; the second command lists
the bundled cabinet (the image has no shell, so inspect it from outside):

```bash
docker run --rm termoso-server:local
# Error: TERMOSO_MASTER_KEY is required (generate with `openssl rand -base64 32`)
docker create --name termoso-inspect termoso-server:local >/dev/null
docker export termoso-inspect | tar -t | grep -E '^app/web/index.html|bin/termoso-server'
docker rm termoso-inspect
```

To run it for real, point the Compose file at your tag instead of the GHCR
image (`image: termoso-server:local` in `deploy/docker-compose.yml`) and follow
[SELF_HOSTING.md](SELF_HOSTING.md). Multi-architecture images (amd64 + arm64)
are produced the same way with `docker buildx build --platform
linux/amd64,linux/arm64 …`; the Dockerfile pins `wasm-pack` checksums for both.

## Server and bridge (native binaries)

If you would rather not use containers:

```bash
cargo build --release --locked -p termoso-server -p termoso-bridge
ls target/release/termoso-server target/release/termoso-bridge
```

`--locked` makes cargo fail instead of silently updating `Cargo.lock`. The
binaries link glibc dynamically and the server additionally needs OpenSSL 3
(`libssl3`, pulled in by the WebAuthn library); the client-facing protocols
use `rustls`. They run on any glibc-based Linux of the same or newer version
than the build host — which is why the container images build on Debian 12.

The server serves the cabinet from `TERMOSO_WEB_DIR`; build the cabinet
([next section](#web-cabinet)) and run:

```bash
export TERMOSO_MASTER_KEY=$(openssl rand -base64 32)   # keep it — it wraps server-side secrets
export TERMOSO_DATABASE_URL=postgres://termoso:termoso@localhost:5432/termoso
export TERMOSO_REDIS_URL=redis://127.0.0.1:6379
export TERMOSO_WEB_DIR=web/dist
target/release/termoso-server
```

Database migrations run automatically at start. The complete variable list is
[`deploy/.env.example`](../deploy/.env.example).

## Web cabinet

The cabinet is a static single-page app; the only "native" piece is the
`termoso-crypto` crate compiled to WebAssembly (OPAQUE, key wrapping, sealed
boxes) — no cryptography is implemented in JavaScript.

```bash
rustup target add wasm32-unknown-unknown
cd web
npm ci
npm run wasm        # crates/termoso-wasm → web/src/crypto/pkg (generated, git-ignored)
npm run build       # → web/dist
```

`web/dist` is what the server image ships under `/app/web`. Serve it from
`termoso-server` (`TERMOSO_WEB_DIR=/path/to/web/dist`) rather than from a
separate static host: the SPA expects the API on the same origin under
`/api/v1`, and the server adds the security headers and the landing/cabinet
routing described in [SELF_HOSTING.md](SELF_HOSTING.md#landing-page-on-off-or-on-its-own-domain).

## Desktop

The desktop app is Tauri 2: a Rust binary (`apps/desktop/src-tauri`, a member
of the Cargo workspace) that embeds the React front end from
`apps/desktop/dist`. `npm run tauri build` runs `npm run build` for the front
end, `cargo build --release` for the binary and then the bundler for the
installers you ask for.

### Update signing key

The official builds embed a [minisign](https://jedisct1.github.io/minisign/)
public key (`apps/desktop/src-tauri/tauri.conf.json` →
`plugins.updater.pubkey`) and the bundler signs every installer with the
matching private key (`bundle.createUpdaterArtifacts: true`). The in-app
updater refuses anything not signed by the embedded key. This has two
consequences for a build from source:

1. **The bundler needs *a* private key.** Without
   `TAURI_SIGNING_PRIVATE_KEY` in the environment `npm run tauri build` fails
   after compiling, with
   `A public key has been found, but no private key. Make sure to set TAURI_SIGNING_PRIVATE_KEY environment variable.`
2. **Your build cannot update from the project's feed** (its signatures are
   made with the project key, your binary carries yours), and the project's
   binaries cannot update from your feed. That is the point of the key.

Pick one of the two ways out:

**A. Your own key (recommended if you distribute the build).** Generate a key
pair once, embed *your* public key and sign with *your* private key:

```bash
cd apps/desktop
npm ci
npm run tauri signer generate -- -w ~/.termoso-signing/termoso.key   # asks for a password
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.termoso-signing/termoso.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD='…the password…'
cat > /tmp/updater.json <<EOF
{ "plugins": { "updater": {
    "pubkey": "$(cat ~/.termoso-signing/termoso.key.pub)",
    "endpoints": ["https://updates.example.com/termoso/latest.json"] } } }
EOF
npm run tauri build -- --config /tmp/updater.json --bundles deb,rpm,appimage
```

`--config` merges the JSON over `tauri.conf.json` for this run only; nothing
in the tree changes. Keep the `.key` file and its password offline: a lost
key means every installed copy of your build has to be reinstalled by hand.
Publishing your own `latest.json` is described in
[RELEASING.md → Self-hosting the update feed](RELEASING.md#self-hosting-the-update-feed).

**B. No updater artifacts (fine for personal use).** Turn the signing step off
for this run with the overlay shipped in the tree,
`apps/desktop/src-tauri/tauri.no-updater.conf.json` (it only sets
`bundle.createUpdaterArtifacts: false`). The installers come out without
`.sig` files. The binary still carries the project's public key and feed URL,
so *Check for updates* keeps working — and would replace your build with the
official one when a newer release exists; set *Check for updates* to *Only
when I ask* or point *Release feed* elsewhere in Settings → Updates if you do
not want that.

```bash
cd apps/desktop
npm ci
npm run tauri build -- --config src-tauri/tauri.no-updater.conf.json --bundles deb,rpm,appimage
```

The examples below use variant B for brevity; replace the `--config` argument
by `/tmp/updater.json` and export the two environment variables for variant A.

### Linux

The release builds run on Ubuntu 22.04; anything with WebKitGTK 4.1 works.
System libraries the Rust side links against (WebKitGTK 4.1, GTK 3, the
tray/appindicator, libsoup 3, librsvg, libudev for the serial port
enumeration) plus the tools the AppImage bundler needs:

```bash
# Debian / Ubuntu
sudo apt-get install -y build-essential curl wget file patchelf pkg-config libssl-dev \
  libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
  libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libudev-dev

# Fedora
sudo dnf install -y @development-tools curl wget file patchelf openssl-devel \
  webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel \
  libsoup3-devel javascriptcoregtk4.1-devel systemd-devel
```

Build:

```bash
cd apps/desktop
npm ci
npm run tauri build -- --config src-tauri/tauri.no-updater.conf.json --bundles deb,rpm,appimage
```

Output (`target/` is the workspace target directory, two levels up):

```text
target/release/termoso-desktop                                  # the binary itself
target/release/bundle/deb/Termoso_0.3.0_amd64.deb
target/release/bundle/rpm/Termoso-0.3.0-1.x86_64.rpm
target/release/bundle/appimage/Termoso_0.3.0_amd64.AppImage
```

The AppImage is fully self-contained apart from `libwebkit2gtk-4.1-0` and
`libgtk-3-0`, which every current desktop distribution has; the `.deb`
declares them as dependencies. On an arm64 host the same command produces
`_arm64` / `.aarch64` packages — there is no cross-compilation, build on the
architecture you target.

`--bundles` accepts any subset of `deb,rpm,appimage`; leave it out to build
all three. Building the AppImage needs `file` and `patchelf` and downloads
`linuxdeploy` on first use.

### Windows

Tauri's [Windows prerequisites](https://tauri.app/start/prerequisites/#windows):
Microsoft C++ Build Tools (the *Desktop development with C++* workload, or
Visual Studio 2022 with it), WebView2 (already part of Windows 10 1803+/11;
the installers download the bootstrapper otherwise), Rust from
[rustup](https://rustup.rs) with the default `x86_64-pc-windows-msvc` host,
and Node 22.

In *PowerShell*:

```powershell
cd apps\desktop
npm ci
npm run tauri build -- --config src-tauri\tauri.no-updater.conf.json --bundles msi,nsis
```

Output:

```text
target\release\termoso-desktop.exe
target\release\bundle\msi\Termoso_0.3.0_x64_en-US.msi
target\release\bundle\nsis\Termoso_0.3.0_x64-setup.exe
```

WiX (for `.msi`) and NSIS (for `-setup.exe`) are downloaded by the bundler on
first use. The installers are not Authenticode-signed unless you configure
`bundle.windows.certificateThumbprint` or `signCommand` in a `--config`
overlay; SmartScreen will show the usual "unknown publisher" prompt for an
unsigned installer. With variant A both installers get a `.sig`; the official
`latest.json` points at the NSIS one (the release workflow sets
`updaterJsonPreferNsis`), so do the same in yours.

The desktop app talks to the system SSH agent through the OpenSSH agent named
pipe (`\\.\pipe\openssh-ssh-agent`) and to Pageant; neither needs anything at
build time.

### macOS

Xcode command-line tools (`xcode-select --install`) and Rust from rustup. One
Mac builds both CPU architectures; add the target you are *not* running on:

```bash
rustup target add x86_64-apple-darwin      # on Apple Silicon
rustup target add aarch64-apple-darwin     # on Intel
cd apps/desktop
npm ci
npm run tauri build -- --config src-tauri/tauri.no-updater.conf.json --bundles app,dmg --target aarch64-apple-darwin
npm run tauri build -- --config src-tauri/tauri.no-updater.conf.json --bundles app,dmg --target x86_64-apple-darwin
```

Output (per `--target`):

```text
target/aarch64-apple-darwin/release/bundle/macos/Termoso.app
target/aarch64-apple-darwin/release/bundle/dmg/Termoso_0.3.0_aarch64.dmg
target/x86_64-apple-darwin/release/bundle/dmg/Termoso_0.3.0_x64.dmg
```

`apps/desktop/src-tauri/tauri.macos.conf.json` is merged automatically on
macOS and sets `signingIdentity: "-"` — an **ad-hoc** signature, so the app
runs on Apple Silicon (which refuses unsigned code) without an Apple developer
account. A copy downloaded through a browser is quarantined by Gatekeeper; the
person installing it runs `xattr -cr /Applications/Termoso.app` once (or
right-click → *Open*). To sign with a Developer ID and notarize instead, export
`APPLE_SIGNING_IDENTITY`, `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`
and, for notarization, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` before
the build — exactly the variables the release workflow uses, see
[RELEASING.md → macOS](RELEASING.md#macos-signing-and-gatekeeper).

A `universal-apple-darwin` target (one binary for both architectures) works
too but doubles the download; the project ships two `.dmg`s instead.

## Android

The Android app is Kotlin/Jetpack Compose over the same Rust core as iOS
(`crates/termoso-mobile`, exposed through UniFFI). Gradle drives everything:
the `:core` module runs `cargo ndk` for each ABI, generates the Kotlin
bindings from the built library and packages them into an AAR; `:app` is the
application.

### SDK and toolchain

Either install Android Studio and let it manage the SDK, or use the
command-line tools only:

```bash
export ANDROID_HOME=$HOME/Android/Sdk
mkdir -p "$ANDROID_HOME/cmdline-tools"
# download "Command line tools only" from https://developer.android.com/studio#command-line-tools-only,
# unzip so that $ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager exists, then:
yes | "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" --licenses
"$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" \
  "platform-tools" "platforms;android-36" "build-tools;36.0.0" "ndk;27.2.12479018"

rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
```

`ANDROID_HOME` must be exported for Gradle; the `:core` module picks the NDK
from `ANDROID_NDK_HOME` if set, otherwise the newest one under
`$ANDROID_HOME/ndk`. JDK 17 must be the `java` on `PATH` or `JAVA_HOME`.

### Debug build

```bash
cd apps/android
./gradlew :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

The debug APK is signed with the standard Android debug key and contains the
ABIs from `gradle.properties` → `termoso.abis` (default `arm64-v8a,x86_64`:
devices plus the x86_64 emulator).

### Release build

A release APK must be signed or Android refuses to install it. The project
reads the keystore from the environment only — nothing in the tree — so
create one once:

```bash
keytool -genkeypair -keystore ~/termoso-release.jks -alias termoso \
  -keyalg RSA -keysize 4096 -validity 10950
```

Then build one APK per ABI plus a universal one, the way the release workflow
does:

```bash
cd apps/android
export TERMOSO_ANDROID_KEYSTORE=~/termoso-release.jks
export TERMOSO_ANDROID_KEYSTORE_PASSWORD='…'
export TERMOSO_ANDROID_KEY_ALIAS=termoso                       # default
# export TERMOSO_ANDROID_KEY_PASSWORD='…'                     # defaults to the store password
./gradlew -Ptermoso.abis=arm64-v8a,armeabi-v7a,x86_64 -Ptermoso.splits=true :app:assembleRelease
ls app/build/outputs/apk/release/
```

```text
app-arm64-v8a-release.apk     # most phones and tablets
app-armeabi-v7a-release.apk   # old 32-bit devices
app-x86_64-release.apk        # emulators, Chromebooks
app-universal-release.apk     # all three ABIs in one file
```

Without `-Ptermoso.splits=true` you get a single `app-release.apk` with the
ABIs of `termoso.abis`. Without `TERMOSO_ANDROID_KEYSTORE` Gradle still builds
but leaves the files as `*-release-unsigned.apk`; sign those afterwards with
`apksigner` if you prefer to keep signing out of the build:

```bash
"$ANDROID_HOME/build-tools/36.0.0/apksigner" sign --ks ~/termoso-release.jks --ks-key-alias termoso \
  --out termoso-arm64-v8a.apk app/build/outputs/apk/release/app-arm64-v8a-release-unsigned.apk
"$ANDROID_HOME/build-tools/36.0.0/apksigner" verify --print-certs termoso-arm64-v8a.apk
```

Two things follow from the signing key:

* **Updates.** Android installs a newer APK over an older one only when both
  are signed with the same key. Users of the official APKs cannot switch to
  yours (or back) without uninstalling. The version code is derived from the
  workspace version (`MAJOR*10000 + MINOR*100 + PATCH`), so your builds of
  newer tags do install over your builds of older ones.
* **App Links.** `gradle.properties` → `termoso.appLinkHost` names the server
  whose `https://<host>/invite/…` and `/join/…` links open directly in the
  app. For a self-hosted server set it to your host
  (`-Ptermoso.appLinkHost=app.example.com`) and publish your certificate's
  SHA-256 through the server's `TERMOSO_ANDROID_APP_LINKS` — details in
  [RELEASING.md](RELEASING.md#secrets). Without it the links still work
  through the web landing page's *Open in Termoso* button.

The unit tests and lint run with `./gradlew :app:testDebugUnitTest :app:lintDebug`.

## iOS

The iOS app is SwiftUI over the same `termoso-mobile` core; there is no
App Store build, the release ships an **unsigned** `.ipa` for sideloading
(AltStore/SideStore/Sideloadly sign it with the installer's Apple ID — see
[IOS_SIDELOAD.md](IOS_SIDELOAD.md)). Requires macOS with Xcode 16 and
`xcodegen`.

```bash
cd apps/ios
./build-core.sh --release --device --no-sim   # Rust → TermosoCoreFFI.xcframework + TermosoCore.swift
xcodegen generate                              # → Termoso.xcodeproj (git-ignored)
xcodebuild archive -project Termoso.xcodeproj -scheme Termoso -configuration Release \
  -destination 'generic/platform=iOS' -archivePath build/Termoso.xcarchive \
  CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=
mkdir -p build/ipa/Payload
cp -R build/Termoso.xcarchive/Products/Applications/Termoso.app build/ipa/Payload/
(cd build/ipa && zip -qry ../Termoso.ipa Payload)
ls -l build/Termoso.ipa
```

For the simulator use `./build-core.sh` (debug, simulator slice) and
`xcodebuild -scheme Termoso -destination 'platform=iOS Simulator,name=iPhone 16'`
or just open the generated project in Xcode. With an Apple developer account
you can instead let Xcode sign and install the app on your own device as
usual; nothing in the project prevents that.

## Verifying a build

* **Server image.** Start PostgreSQL and Redis
  (`docker compose -f deploy/docker-compose.dev.yml up -d --wait` publishes
  them on localhost), then run your image on the host network:

  ```bash
  docker run --rm --network host -e TERMOSO_MASTER_KEY=$(openssl rand -base64 32) termoso-server:local
  curl -s localhost:8080/healthz          # ok
  ```

  `http://localhost:8080/` serves the landing page — proof that the cabinet is
  bundled.
* **Desktop.** Install the package or run `target/release/termoso-desktop`;
  *Settings → About* shows the version. Publish checksums of your installers
  with `sha256sum target/release/bundle/*/*.{deb,rpm,AppImage}` the way the
  release publishes `SHA256SUMS.txt`.
* **Android.** `apksigner verify --print-certs` shows the signing certificate;
  `adb install -r` then *Settings → About* shows the version.
* **Tests.** `scripts/check.sh rust web desktop android` runs what CI runs for
  the components whose toolchain is installed
  ([CONTRIBUTING.md → Checks](../CONTRIBUTING.md#checks)). The Rust
  integration suite expects the dev stack from `deploy/docker-compose.dev.yml`.

## Reproducibility notes

* All three package managers are pinned by lock files committed to the
  repository (`Cargo.lock`, `web/package-lock.json`,
  `apps/desktop/package-lock.json`, `apps/android/gradle/libs.versions.toml` +
  `gradle-wrapper.properties`). Use `--locked` with cargo and `npm ci` (never
  `npm install`) to build exactly what the tag describes.
* The Dockerfiles pin the `wasm-pack` release and its SHA-256, and build on
  `rust:1-bookworm` / `node:22-bookworm-slim`; pin those to a digest if you
  need byte-identical rebuilds.
* Rust binaries are not bit-for-bit reproducible across different toolchain
  versions or build paths. Compare behaviour and checksums of *your own*
  builds, not against the published assets.
* Android release builds run R8 with the rules in `apps/android/app/proguard-rules.pro`;
  the `.so` libraries are stripped (`llvm-strip --strip-all`) so the APK stays
  small. Debug builds keep symbols.

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `A public key has been found, but no private key` at the end of `tauri build` | The updater is configured. Either export your own key (variant A) or pass `--config src-tauri/tauri.no-updater.conf.json` (variant B). |
| `failed to run custom build command for glib-sys / soup3-sys / javascriptcoregtk` | Missing Linux system libraries; install the list under [Linux](#linux). |
| `npm ci` in `web/` or `apps/desktop/` complains about `engines` / Vite fails to start | Node older than 22.12; upgrade (`nvm install 22`). |
| `Cannot find module 'src/crypto/pkg/termoso_wasm'` | `npm run wasm` was not run (or `wasm32-unknown-unknown` target / `wasm-pack` is missing). |
| Gradle: `SDK location not found` / `NDK not configured` | Export `ANDROID_HOME` (and `ANDROID_NDK_HOME` if the NDK is elsewhere); the required NDK is `27.2.12479018`. |
| Gradle: `cargo ndk` not found | `cargo install cargo-ndk` and make sure `~/.cargo/bin` is on the `PATH` Gradle sees. |
| `error: linking with cc failed` for an Android target | The Rust target is not installed: `rustup target add <triple>`. |
| Release APK `INSTALL_FAILED_UPDATE_INCOMPATIBLE` | Different signing key than the installed copy; uninstall first. |
| macOS: "Termoso is damaged and can't be opened" | Ad-hoc signature + Gatekeeper quarantine; `xattr -cr /Applications/Termoso.app`. |
| `cargo build --locked` fails with "lock file needs to be updated" | You changed a dependency without updating `Cargo.lock`; run without `--locked` once and commit the lock file. |
