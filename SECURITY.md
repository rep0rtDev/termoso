# Security policy

Termoso holds people's SSH keys and passwords. If you find a way to weaken
that, we want to know first and quietly.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting:
<https://github.com/termoso/termoso/security/advisories/new>. It creates a
private advisory that only you and the maintainers can see. **Do not open a
public issue or pull request for security problems.**

Include what you can: affected component (server, desktop, Android, iOS, web
cabinet, bridge, crate), version or commit, steps or a proof of concept,
and the impact you believe it has. You will get an acknowledgement within
**3 working days**, an assessment within **10**, and a fix or mitigation
timeline agreed with you. We credit reporters in the release notes unless
you prefer otherwise; there is no bug bounty.

Please give us up to **90 days** before public disclosure; we will ask for
less if the fix is quick.

## Scope

In scope: everything in this repository — the server and its API, all
clients, the cryptographic design in `crates/termoso-crypto` and
`crates/termoso-proto`, the deployment files, the release pipeline. Bugs that
let the server or another user read vault contents, bypass MFA or device
approval, escalate team roles, or make a client trust the wrong SSH host key
are the highest priority.

Out of scope: vulnerabilities in third-party services you self-host next to
Termoso (PostgreSQL, Redis, MinIO, your reverse proxy), social engineering,
and denial of service that needs more traffic than a single machine produces.

## Verifying a release

Every tag published by `release.yml` carries four independent proofs. Use
whichever your policy requires; they do not depend on each other.

1. **Checksums + keyless signature.** `SHA256SUMS.txt` lists every asset;
   `SHA256SUMS.txt.sigstore.json` is a Sigstore bundle produced by
   `cosign sign-blob` with the workflow's OIDC identity (no long-lived key
   to leak). Verify the file, then the asset:

   ```sh
   cosign verify-blob SHA256SUMS.txt \
     --bundle SHA256SUMS.txt.sigstore.json \
     --certificate-identity-regexp '^https://github.com/termoso/termoso/\.github/workflows/release\.yml@refs/tags/v' \
     --certificate-oidc-issuer https://token.actions.githubusercontent.com
   sha256sum -c --ignore-missing SHA256SUMS.txt
   ```

2. **Build provenance (SLSA).** Each binary asset — installers, AppImages,
   `.deb`/`.rpm`, APKs, `.ipa`, SBOMs — has a GitHub build-provenance
   attestation naming the workflow, commit and runner that produced it:

   ```sh
   gh attestation verify Termoso_0.9.1_amd64.AppImage -R termoso/termoso
   ```

3. **Updater signatures.** Desktop updates (`latest.json`, `*.sig`) are
   signed with the Tauri updater key embedded in the app; the client refuses
   an update that does not verify. Android APKs are signed with the release
   keystore (the fingerprint is pinned by the store and by `apps/android`).

4. **Package channels.** The apt/dnf repositories, the Homebrew tap and the
   winget manifest are generated from the same release assets by the
   `distribute` job; their own signatures (repository GPG key, Homebrew
   formula sha256, winget manifest hashes) chain back to `SHA256SUMS.txt`.

## Software bill of materials

Each release ships an SBOM of the exact source tree it was built from, in
SPDX (`termoso-<version>.spdx.json`) and CycloneDX
(`termoso-<version>.cdx.json`), covering Cargo, npm and Gradle dependencies.
Feed it to your scanner of choice (`grype sbom:termoso-<version>.cdx.json`,
Dependency-Track, Snyk). In CI, `cargo deny` and `npm audit` gate every
change on advisories, licences and sources (see "Dependencies" above); the
controls map for auditors lives in [docs/COMPLIANCE.md](docs/COMPLIANCE.md).

## Threat model

What the design protects against, and — just as important — what it does
not. "End-to-end encrypted" covers one class of attack; the rest is listed
so nobody is surprised.

**Protected.** The sync server, its database backups, the object store and
anyone who compromises them see only ciphertext: vault contents (hosts,
keys, passwords, snippets, session logs) are encrypted client-side with a
vault key that exists on the server solely as sealed copies for members'
account keys. The OPAQUE protocol keeps the password off the server; the
account private key is unwrapped only in client memory. A server cannot
substitute the key of your *personal* vault: new personal key versions are
accepted only in a self-authenticated envelope that requires your private
key to produce, and a client never swaps a personal key it already holds for
a same-version envelope. Entity ciphertext is bound to its kind and id, so
records cannot be swapped between hosts or vaults.

**Trusted by design — the server.** It is trusted for *freshness and
ordering*: `version`, `seq` and `key_version` are server-controlled and not
covered by the entity AAD, so a malicious server can withhold changes,
serve an older snapshot of a vault (rollback) or replay deleted records. It
cannot forge or alter a record's content. The server is also the **key
directory for teams**: it tells clients who is a member and which public key
belongs to them. A malicious server or team manager could add a member (or a
key) you did not intend; team vault membership and manager actions are shown
in the audit log for that reason, and personal vaults do not depend on the
directory at all. The server sees **metadata**: who signs in from which IP
and device, when, how many entities each vault has and their sizes, which
teams exist, and the timing of every sync. Run your own server if that
matters to you.

**Not protected — your device.** A compromised, rooted or malware-infected
device can read everything the app can while it is unlocked: the master key
and vault keys are in process memory, and a debugger or memory dump gets
them. App Lock (desktop master password, Android/iOS biometric lock) narrows
the window but does not change this; an attacker with your unlocked device
is you. Browser sessions in the web cabinet keep the account private key in
tab memory and rely on the browser's origin isolation; a malicious extension
or a compromised browser profile can read it.

**Host trust.** SSH host keys and WebDAV TLS certificates are only as good
as the first acceptance. Termoso pins them after you accept (known hosts /
saved certificate fingerprint), shows a security-sensitive prompt with both
fingerprints when one *changes*, never replaces a changed key silently, and
offers a one-time connection instead of saving. Clicking "accept" on an
unknown key without checking it out-of-band lets a man-in-the-middle read
that session — the same as with any SSH client.

**Untrusted input.** Terminal output, SFTP/WebDAV listings, imported
configuration files and SAML responses are attacker-controlled input. The
parsers for them are written in Rust and fuzzed (`fuzz/`); this lowers the
risk but does not remove logic bugs. A remote program can *write* your
clipboard through OSC 52 (as in tmux/vim workflows) but clipboard reads are
never answered; hyperlinks in terminal output open only on modifier-click.

**Cryptographic residual risk.** The RSA implementation used for RSA SSH
keys and SAML assertion decryption has a known timing side channel
(RUSTSEC-2023-0071, "Marvin") with no upstream fix in any released version.
It is exploitable only by an attacker who can time many private-key
operations on your machine; the default key type is Ed25519 and SAML
decryption runs on your own server. The accepted advisory is documented in
`deny.toml` and revisited on every dependency update.

**No telemetry.** Clients talk only to the hosts you configure and, if you
sign in, to the sync server you chose. There are no analytics, crash
reporting or feature-flag SDKs; server metrics are opt-in and stay on your
server. A build that opens any other connection is a bug — report it.

* **Identity providers (SSO).** An OIDC/SAML provider only proves control of
  an e-mail address. A compromised or malicious IdP can therefore sign in to
  the account on a new device, read nothing (the vault key is derived from the
  password, which the IdP never sees) and cannot link itself to another
  account: linking requires either the password login it is attached to or a
  step-up from the signed-in cabinet, and unlinking requires step-up.

## Supported versions

The latest release and `main` receive fixes. Older releases are not patched;
upgrading is the fix.

## What we do on our side

* Every release is built from tagged source on CI; desktop updates are signed
  and the updater is off unless you turn it on.
* Dependencies are updated weekly by Dependabot and reviewed like any change.
  CI fails on RustSec advisories, licences outside the allow-list, crates
  from unknown registries or git sources and banned legacy TLS/HTTP stacks
  (`deny.toml`), and on `npm audit` findings of moderate severity or higher
  in the web cabinet and desktop webview. The full transitive Gradle graph of
  the Android app is submitted to GitHub so Dependabot alerts cover it too.
  Every accepted advisory carries a written reason in `deny.toml`.
* The one accepted vulnerability advisory (the others are unmaintained
  compile-time crates) is RUSTSEC-2023-0071 ("Marvin", a timing
  side channel in the pure-Rust `rsa` crate's private-key arithmetic, with no
  fixed release). The server does not use that crate for private-key
  operations: the SAML SP key — signing `AuthnRequest`s and unwrapping
  encrypted assertions, the only RSA private-key operation an outsider can
  trigger (through the ACS) — runs in AWS-LC, whose RSA is constant-time.
  `rsa` remains for parsing and for public-key signature checks, where the
  advisory does not apply. What remains: an SSH client signing a userauth
  challenge with an RSA key does so on the user's own device (Ed25519 is the
  default), so the timing is not remotely observable. This narrows the
  exposure; it is not a claim that RSA timing is a solved problem everywhere
  in the dependency tree.
* Cryptographic choices are documented in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md);
  if you believe one of them is wrong, that is a valid report too.
