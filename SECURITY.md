# Security policy

Termoso holds people's SSH keys and passwords. If you find a way to weaken
that, we want to know first and quietly.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting:
<https://github.com/rep0rtDev/termoso/security/advisories/new>. It creates a
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
* Cryptographic choices are documented in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md);
  if you believe one of them is wrong, that is a valid report too.
