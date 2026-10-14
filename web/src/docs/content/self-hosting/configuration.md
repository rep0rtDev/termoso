---
title: Configuration
description: The environment variables that matter, the reverse proxy, domains, e-mail, passkeys and SSO.
---

The server reads `TERMOSO_*` environment variables; nested sections use a double underscore (`TERMOSO_S3__ENDPOINT`). The complete annotated list is [`deploy/.env.example`](https://github.com/rep0rtDev/termoso/blob/main/deploy/.env.example). The server validates its configuration on start and refuses to run with an invalid one.

## Minimum

```dotenv
TERMOSO_MASTER_KEY=            # openssl rand -base64 32 — back it up with the database
TERMOSO_PUBLIC_URL=https://termoso.example.com
POSTGRES_PASSWORD=             # openssl rand -hex 24
RUSTFS_ACCESS_KEY=termoso
RUSTFS_SECRET_KEY=             # openssl rand -hex 24
TERMOSO_S3__PUBLIC_ENDPOINT=https://s3.termoso.example.com
TERMOSO_ADMIN_EMAILS=you@example.com
```

`TERMOSO_MASTER_KEY` wraps the server-side secrets (SSO client secrets, bridge tokens, WebAuthn state). A database restored with a different master key is unusable — store the key next to the backups.

## E-mail

```dotenv
TERMOSO_SMTP__HOST=smtp.example.com
TERMOSO_SMTP__PORT=587
TERMOSO_SMTP__USERNAME=…
TERMOSO_SMTP__PASSWORD=…
TERMOSO_SMTP__SECURITY=starttls
TERMOSO_SMTP__FROM=Termoso <no-reply@example.com>
```

Without SMTP the server still runs, but e-mail verification, device approval by e-mail, e-mail MFA and team digests are off.

## Reverse proxy

The API and the cabinet share one origin. Forward everything to `api:8080`, including WebSocket upgrades on `/api/v1/ws`, and set `X-Forwarded-For`. The Compose `proxy` profile does this with Caddy:

```dotenv
TERMOSO_DOMAIN=termoso.example.com
TERMOSO_S3_DOMAIN=s3.termoso.example.com
ACME_EMAIL=you@example.com
```

Never enable `TERMOSO_TRUST_PROXY` on a server that is reachable without the proxy in front.

## Landing page and domains

| Layout               | Settings                                                                                                                               | Result                                                                                        |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| One origin (default) | `TERMOSO_PUBLIC_URL=https://termoso.example.com`                                                                                       | `/` is the landing, `/docs` the docs, `/login` the cabinet                                    |
| No landing           | `TERMOSO_LANDING=false`                                                                                                                | `/` goes straight to sign-in                                                                  |
| Landing on the apex  | `TERMOSO_PUBLIC_URL=https://app.example.com`<br>`TERMOSO_WEB_URL=https://app.example.com`<br>`TERMOSO_LANDING_URL=https://example.com` | `example.com` serves the landing and `/docs`; every other path redirects to `app.example.com` |

## Passkeys, App Links, SSO

- `TERMOSO_WEBAUTHN__RP_ID` must be the registrable domain of `TERMOSO_PUBLIC_URL`. Keep the `tauri://localhost,http://tauri.localhost` origins so the desktop app can use passkeys.
- `TERMOSO_ANDROID_APP_LINKS=<package>=<SHA-256>` publishes `/.well-known/assetlinks.json` so invitation and join links open in the Android app.
- SSO providers are `TERMOSO_SSO__<slug>__*`: OpenID Connect (any discovery URL, presets for Google and Microsoft, GitHub over OAuth2) or SAML 2.0 (`KIND=saml`). The OIDC redirect URI is `<TERMOSO_PUBLIC_URL>/api/v1/auth/sso/callback`.

## Optional features

| Setting                    | Feature                                                       |
| -------------------------- | ------------------------------------------------------------- |
| `TERMOSO_S3__*`            | Session logs stored on the server                             |
| `TERMOSO_AI__*`            | Opt-in AI command suggestions through your own model endpoint |
| `TERMOSO_SSHID_URL`        | SSH ID on a dedicated origin                                  |
| `TERMOSO_METRICS__ENABLED` | Prometheus metrics on a separate, unpublished listener        |
| `TERMOSO_SWAGGER_UI`       | Swagger UI at `/api/docs`                                     |
