# Releasing the desktop app

Desktop builds are produced by `.github/workflows/release.yml`. Nothing is
built on a developer machine and no private key ever leaves GitHub Actions.

## What a release contains

| Platform | Installers | Updater artifact |
|---|---|---|
| Linux | `.deb`, `.rpm`, `.AppImage` | `*.AppImage` + `*.AppImage.sig` |
| Windows | NSIS `*-setup.exe`, `*.msi` | `*-setup.exe` + `.sig` (preferred), `*.msi` + `.sig` |

plus:

* `latest.json` — the updater manifest: version, release notes, publication
  date and, per platform target, the artifact URL and its signature;
* `SHA256SUMS.txt` — checksums of every asset above.

Signatures are [minisign](https://jedisct1.github.io/minisign/) signatures
made with the project signing key. The matching public key is compiled into
the app (`apps/desktop/src-tauri/tauri.conf.json` → `plugins.updater.pubkey`),
and the updater refuses any artifact whose signature does not verify — a
compromised web server or mirror cannot push code to users.

## Cutting a release

1. Bump the version in **both** `apps/desktop/package.json` and the workspace
   `Cargo.toml` (the workflow fails if they differ, or if the tag does not
   match them). Run `cargo check -p termoso-desktop` so `Cargo.lock` follows.
2. Merge to `main`, then tag and push:

   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

   A tag with a pre-release suffix (`v0.2.0-rc.1`) is marked *pre-release*
   and never becomes `releases/latest`, so it is invisible to the default
   updater feed.
3. The workflow builds Linux and Windows in parallel, signs the artifacts,
   assembles a **draft** release, verifies that every bundle and signature is
   present, adds `SHA256SUMS.txt` and only then publishes the release. Until
   that last step `releases/latest/download/latest.json` still points at the
   previous version, so clients never see a half-uploaded release.

`workflow_dispatch` (the **Run workflow** button) builds the same bundles from
any branch and attaches them to the workflow run as artifacts without
creating a release — use it to smoke-test packaging changes.

## Secrets

| Secret | Purpose |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | minisign private key (contents of the `.key` file) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | its password |

Generate a key pair once with

```bash
npx tauri signer generate -w ~/.termoso-signing/termoso.key
```

store the private key and password as repository Actions secrets, keep an
offline backup (losing the key means shipping a new key and asking every
user to reinstall), and paste the `.key.pub` contents into
`plugins.updater.pubkey`. Rotating the key requires one release signed with
the *old* key whose binary already carries the *new* public key.

## How the updater behaves

* **Off by default.** The app never contacts any server unless the user
  clicks *Check for updates* or turns on *Check on startup* in Settings →
  Updates. There is no background polling and no telemetry of any kind in the
  request — it is a plain `GET` of `latest.json`.
* **Explicit install.** A found update is shown with its notes; download and
  installation start only when the user asks, and the app restarts only when
  the user confirms.
* **Verified.** The downloaded artifact must carry a valid signature for the
  embedded public key, and the feed URL must be `https://` — anything else is
  rejected before a request is made.
* **Rust-owned.** Checking, downloading and verification live in
  `apps/desktop/src-tauri/src/update.rs`; the webview only renders progress and
  metadata and never sees keys or artifact bytes.

## Self-hosting the update feed

The default feed is `https://github.com/rep0rtDev/termoso/releases/latest/download/latest.json`.
To keep updates entirely on infrastructure you control:

1. Download `latest.json`, the updater artifacts and their `.sig` files from
   the release and put them on any HTTPS static host (the same server that
   runs `termoso-server`, an S3 bucket, nginx — anything that serves files).
2. Rewrite the `url` fields inside `latest.json` to point at your copies.
   Signatures stay as they are: they cover the artifact bytes, not the URL.
3. In the app, Settings → Updates → *Release feed*, enter the URL of your
   `latest.json`. Every device using that feed now updates only from your
   server, still verifying against the project public key.

If you also fork the project and build your own binaries, generate your own
signing key (above), embed its public key and publish your own `latest.json`.
Clients of a fork trust the fork's key only.

`latest.json` looks like this:

```json
{
  "version": "0.2.0",
  "notes": "Signed desktop builds of Termoso 0.2.0.",
  "pub_date": "2026-09-10T17:00:00Z",
  "platforms": {
    "linux-x86_64": {
      "url": "https://updates.example.com/termoso/0.2.0/Termoso_0.2.0_amd64.AppImage",
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6…"
    },
    "windows-x86_64": {
      "url": "https://updates.example.com/termoso/0.2.0/Termoso_0.2.0_x64-setup.exe",
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6…"
    }
  }
}
```
