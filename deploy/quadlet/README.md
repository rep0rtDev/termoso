# Podman Quadlet units

The Compose stack as systemd services. Install, configure and operate them as
described in [docs/SELF_HOSTING.md → Podman Quadlet](../../docs/SELF_HOSTING.md#podman-quadlet).

```bash
sudo install -d -m 0750 /etc/termoso
sudo install -m 0600 ../.env               /etc/termoso/env        # filled-in copy of ../.env.example
sudo install -m 0600 stack.env.example     /etc/termoso/stack.env  # then set the passwords
sudo cp *.container *.network *.volume     /etc/containers/systemd/
sudo systemctl daemon-reload && sudo systemctl start termoso-api
```

Validate after editing: `/usr/libexec/podman/quadlet -dryrun` (or
`/usr/lib/podman/quadlet` depending on the distribution).
