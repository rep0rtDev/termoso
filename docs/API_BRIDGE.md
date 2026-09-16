# API Bridge

The API Bridge lets your automation (Ansible, Terraform, a CMDB, an autoscaler)
create, update and delete hosts and groups in a Termoso vault over a plain REST
API — without ever sending plaintext to the Termoso server.

Termius sells the same idea as a paid Team feature. Ours is free, and unlike
theirs the server side stays zero-knowledge: the bridge runs **in your
infrastructure**, holds the vault keys, encrypts everything locally and pushes
only ciphertext through the ordinary sync API. The Termoso server (cloud or
self-hosted) sees the bridge as one more device that syncs opaque blobs.

```
 your automation ──REST (plaintext, your network)──▶ termoso-bridge ──sync (ciphertext)──▶ Termoso server
                                                    (vault keys live here)                (sees only blobs)
```

## How it works

1. In the web cabinet (**API Bridge** page, `/bridges`) you create a bridge
   and choose which vaults it may write — any vault where you are an editor or
   manager and hold the current key. Your browser generates an X25519 key
   pair for the bridge, seals each vault key to its public key and uploads
   only the sealed keys plus the public key. It downloads a **credentials
   file** once — the server never sees the private key. The same page lists
   bridges with their last use, edits/re-seals vault sets and revokes them.
2. The bridge container starts with that file, logs in with its own token
   (a separate, revocable session that can only call `/sync/*` and
   `/bridge/me`), opens the sealed vault keys locally and mirrors the vaults.
3. Every REST call is translated into encrypted entity changes and pushed with
   the normal conflict-checked sync. Your desktop and mobile clients pick the
   changes up like any other sync.

If a vault key is rotated, the bridge's sealed key is dropped and writes to
that vault answer `409 vault_key_pending` until the owner re-seals it in the
cabinet. The bridge cannot read entities encrypted under a key version it does
not hold; the owner's client re-encrypts them on rotation.

Revoking the bridge in the cabinet kills its token immediately. The bridge
never receives the account password, recovery key or any other vault's key.

## Running the bridge

```bash
docker run -d --name termoso-bridge --restart unless-stopped \
  -p 127.0.0.1:8080:8080 \
  -v /srv/termoso/bridge.json:/etc/termoso/bridge.json:ro \
  -e TERMOSO_BRIDGE_API_KEY="$(openssl rand -hex 32)" \
  ghcr.io/rep0rtdev/termoso-bridge:latest
```

| Variable | Default | Meaning |
|---|---|---|
| `TERMOSO_BRIDGE_CREDENTIALS` | `/etc/termoso/bridge.json` | credentials file downloaded from the cabinet |
| `TERMOSO_BRIDGE_LISTEN` | `0.0.0.0:8080` | bind address of the local REST API |
| `TERMOSO_BRIDGE_API_KEY` | *(none)* | shared secret callers must present (`Authorization: Bearer …` or `X-Api-Key`). Strongly recommended unless bound to loopback |
| `TERMOSO_BRIDGE_SYNC_INTERVAL` | `60` | seconds between background refreshes (`0` disables) |
| `TERMOSO_BRIDGE_RATE_LIMIT` | `50` | sustained requests/second on `/v1` (burst 2×, `0` disables) |
| `RUST_LOG` | `info` | log filter. Logs never contain addresses, usernames, passwords or keys |

The credentials file is the only secret the bridge needs. Treat it like an SSH
private key: `chmod 600`, mount read-only, never commit it.

The image is distroless and runs as `nonroot`. Build it yourself with
`docker build -f deploy/Dockerfile.bridge .` or run the binary directly:
`cargo run --release -p termoso-bridge`.

## REST API

Paths follow the Termius API Bridge so existing playbooks port with a URL
change. Trailing slashes are optional; `POST` and `PUT` both mean *create or
update*. Every mutation is idempotent on `external_id` — your inventory id
(`i-0abc…`, an Ansible inventory name, a CMDB key). Repeating a request
changes nothing; changing a field updates the same host in place.

| Method | Path | |
|---|---|---|
| `GET` | `/healthz` | liveness (no auth) |
| `GET` | `/v1/bridge/me/` | bridge status: vaults, readiness, counts |
| `POST` | `/v1/sync/` | refresh assignment and pull now |
| `GET` | `/v1/vaults/` | writable vaults |
| `GET` | `/v1/hosts/?vault=` | hosts managed through the bridge |
| `GET` | `/v1/host/{external_id}/` | one host |
| `POST`/`PUT` | `/v1/host/{external_id}/` | create or update |
| `DELETE` | `/v1/host/{external_id}/` | delete (with its configs and inline credentials) |
| `GET` | `/v1/groups/?vault=` | groups |
| `GET` | `/v1/group/{external_id}/` | one group |
| `POST`/`PUT` | `/v1/group/{external_id}/` | create or update |
| `DELETE` | `/v1/group/{external_id}/` | delete; hosts and sub-groups move to the parent |

`?vault=` (name or id) is required only when the bridge writes to more than one
vault. Errors are JSON `{ "code", "message" }`: `invalid_request` 400,
`unauthorized` 401, `not_found` 404, `vault_key_pending`/`conflict` 409,
`rate_limited` 429 (with `Retry-After`), `server_unreachable`/`server_error`
502.

### Host

```jsonc
PUT /v1/host/i-0123456789/
{
  "vault": "Production",          // optional when only one vault is assigned
  "group": "eu-west-1",           // external_id of a group created via the bridge
  "address": "10.0.0.5",
  "label": "db-1",                // defaults to the address
  "tags": ["db", "postgres"],     // matched by label, created when missing
  "os": "Ubuntu 24.04",
  "notes": "managed by ansible",
  "ssh": {
    "port": 22,
    "credentials": {
      "username": "ubuntu",
      "password": "…",            // optional
      "key": {                    // optional
        "private": "-----BEGIN OPENSSH PRIVATE KEY-----\n…",
        "public": "ssh-ed25519 AAAA… ansible",
        "passphrase": "…",
        "label": "ansible deploy key"
      }
    }
  },
  "telnet": { "port": 23, "credentials": { "username": "…", "password": "…" } }
}
```

The response is a summary and **never echoes credentials**:

```json
{ "id": "…", "external_id": "i-0123456789", "vault": "Production", "vault_id": "…",
  "group": "eu-west-1", "label": "db-1", "address": "10.0.0.5", "tags": ["db", "postgres"],
  "ssh_port": 22, "telnet_port": null, "has_credentials": true }
```

Semantics: `address`, `label`, `group`, `tags`, `ssh` and `telnet` describe the
host and are replaced on every call (omit `ssh` to remove the SSH config, omit
`credentials` to drop them). Fields people set in the app and the bridge does
not model — icon, IP version, startup snippet, sort order, detected OS — are
left alone; `notes` and `os` are only overwritten when present. Credentials are
stored as an inline identity of the host (not listed in the Keychain) and are
removed together with the host.

### Group

```jsonc
PUT /v1/group/eu-west-1/
{
  "vault": "Production",
  "parent": "aws",                // optional external_id of another group
  "label": "eu-west-1",
  "ssh": { "port": 22, "credentials": { "username": "ubuntu", "key": { "private": "…" } } }
}
```

Hosts in the group inherit its SSH/Telnet settings and credentials, as in the
apps.

### Example: Ansible

```yaml
- name: Register hosts in Termoso
  hosts: localhost
  tasks:
    - uri:
        url: "http://127.0.0.1:8080/v1/host/{{ item.instance_id }}/"
        method: PUT
        headers: { Authorization: "Bearer {{ termoso_bridge_key }}" }
        body_format: json
        body:
          group: "{{ item.tags.env }}"
          address: "{{ item.private_ip_address }}"
          label: "{{ item.tags.Name }}"
          tags: ["aws", "{{ item.placement.availability_zone }}"]
          ssh: { credentials: { username: ubuntu, key: { private: "{{ lookup('file', 'deploy.key') }}" } } }
      loop: "{{ ec2_instances }}"
```

### Limits

Bodies up to 256 KiB; labels 200 characters; addresses 253; notes 4 KiB; 32
tags per host; private keys 64 KiB. Requests that fail validation write
nothing.

## Threat model in one paragraph

Whoever can reach the bridge's port with the API key can write to the assigned
vaults and read their host list (never credentials). Whoever obtains the
credentials file can do the same **and** read the assigned vaults, so keep it
off shared file systems and revoke the bridge in the cabinet if it leaks. The
Termoso server, its operators and anyone with its database see only ciphertext
and the metadata every sync client already exposes (entity ids, kinds,
versions, timestamps, the bridge's last-used time and IP).
