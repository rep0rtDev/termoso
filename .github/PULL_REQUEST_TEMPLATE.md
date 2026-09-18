## What

<!-- One paragraph: what changes for a user or operator, and why. Link the issue if there is one. -->

## How

<!-- Anything a reviewer cannot get from the diff alone: design choices, alternatives rejected, migrations, protocol/entity changes, config keys added. -->

## Testing

<!-- What you ran (scripts/check.sh areas, manual steps, platforms). UI: screenshots or a short recording. -->

## Checklist

- [ ] `scripts/check.sh` passes for the areas touched
- [ ] No telemetry, no new network calls the user did not ask for
- [ ] Vault data stays encrypted client-side; the server gained no way to read it
- [ ] New config keys are documented in `deploy/.env.example` and the README
- [ ] Database changes are a new migration file
- [ ] Docs updated (README / docs/) where behaviour changed
