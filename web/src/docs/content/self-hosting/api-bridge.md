---
title: API Bridge
description: Let Ansible, Terraform or a CMDB manage hosts in a vault over a REST API — without the server ever seeing plaintext. Free on the hosted cloud and on self-hosted servers.
---

The API Bridge is a small container you run **in your own infrastructure**. It holds the keys of the vaults you give it, exposes a Termius-compatible REST API for hosts and groups, encrypts everything locally and pushes only ciphertext through the ordinary sync API. To the server it is one more device.

> [!NOTE]
> The bridge runs on your side; nothing is hosted per user. It is offered by the hosted cloud at `app.termoso.dev` and by self-hosted servers unless the operator sets `TERMOSO_BRIDGES=false`.

## Setting it up

1. In the cabinet, open **API Bridge → New bridge**, name it and pick the vaults it may write to. The key pair is generated in your browser and each vault key is sealed to it there.
2. Download the credentials file. It is shown **once** and contains the bridge's private key and token.
3. Run the container next to your automation:

```bash
docker run -d --name termoso-bridge --restart unless-stopped \
  -p 127.0.0.1:8081:8080 \
  -v "$PWD/termoso-bridge.json:/etc/termoso/bridge.json:ro" \
  -e TERMOSO_BRIDGE_API_KEY="$(openssl rand -hex 32)" \
  ghcr.io/rep0rtdev/termoso-bridge:latest
```

With Docker Compose use the `bridge` profile: `docker compose -f deploy/docker-compose.yml --profile bridge up -d`.

## Security

- Keep the port bound to localhost or set `TERMOSO_BRIDGE_API_KEY`; callers then send it as a bearer token.
- Revoking the bridge in the cabinet kills its token at once. After a vault key rotation, the bridge needs the new key re-sealed from the cabinet before it can write again.
- The server never receives host addresses, usernames, passwords or keys from the bridge — only ciphertext, like from any other device.

Full API reference: [docs/API_BRIDGE.md](https://github.com/rep0rtDev/termoso/blob/main/docs/API_BRIDGE.md).
