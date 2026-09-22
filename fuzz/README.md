# Fuzzing

libFuzzer targets (via [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz))
for every parser that consumes bytes from a party we do not trust: the IdP,
the SSH/WebDAV server the user connected to, pasted files, deep links.

| Target               | Input                                                        |
| -------------------- | ------------------------------------------------------------ |
| `saml_response`      | base64 `SAMLResponse` posted to the ACS (c14n, DSig, XML-Enc, conditions) |
| `saml_metadata`      | IdP metadata document                                        |
| `saml_xml`           | XML-DSig / XML-Enc / c14n primitives on arbitrary XML        |
| `webdav_multistatus` | PROPFIND `207 Multi-Status` bodies                           |
| `webdav_path`        | path / URL / fingerprint normalisation (`..` must never escape the root) |
| `terminal_feed`      | remote shell output through the mobile terminal emulator     |
| `ssh_keys`           | pasted/imported private keys, certificates, public lines (OpenSSH, PEM, PuTTY) |
| `quick_target`       | quick-connect strings, `ssh://` / `telnet://` deep links, server URL normaliser |

```sh
rustup toolchain install nightly --component rust-src
cargo install cargo-fuzz

cd fuzz
cargo +nightly run --example seed          # well-formed seed corpora
cargo +nightly fuzz check                  # type-check all targets
cargo +nightly fuzz run saml_response -- -max_total_time=900 -max_len=16384 -timeout=10 -rss_limit_mb=2048
```

Any crash, timeout or OOM is written to `artifacts/<target>/`; reproduce with
`cargo +nightly fuzz run <target> artifacts/<target>/<file>`. The `fuzz`
package is excluded from the workspace so the stable toolchain and
`cargo deny` never see it; CI type-checks the targets and runs a short smoke
of each one on nightly.
