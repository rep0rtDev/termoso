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
