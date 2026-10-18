#!/usr/bin/env bash
# Restore drill: bring up a throwaway stack, restore a backup into it, check
# that the API comes up on the restored data and that a login works.
#
#   deploy/restore-test.sh <backup.dump[.age]> [email password]
#
# Needs: docker compose, the same TERMOSO_MASTER_KEY as the backup (in
# deploy/.env or the environment), TERMOSO_BACKUP_AGE_IDENTITY for encrypted
# dumps. Uses a separate Compose project (termoso-restore-test) so the
# production stack is untouched; tears it down at the end.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
dump="${1:?usage: restore-test.sh <backup.dump[.age]> [email password]}"
email="${2:-}"; password="${3:-}"
project="termoso-restore-test"
compose=(docker compose -f "$here/docker-compose.yml" -p "$project" --env-file "${TERMOSO_ENV_FILE:-$here/.env}")
port="${RESTORE_TEST_PORT:-18080}"
export TERMOSO_PORT="$port" TERMOSO_LISTEN=127.0.0.1 RUSTFS_PORT=19000 RUSTFS_CONSOLE_PORT=19001
export TERMOSO_BACKUP_DIR="$(cd "$(dirname "$dump")" && pwd)"
file="/backups/$(basename "$dump")"

cleanup() { "${compose[@]}" down -v --remove-orphans >/dev/null 2>&1 || true; }
trap cleanup EXIT

echo "▸ empty stack"
"${compose[@]}" up -d postgres redis rustfs --wait
echo "▸ restore $dump"
args=(restore --from "$file" --yes)
[ -n "${TERMOSO_BACKUP_AGE_IDENTITY:-}" ] && args+=(--age-identity "/backups/$(basename "$TERMOSO_BACKUP_AGE_IDENTITY")")
"${compose[@]}" --profile backup run --rm backup "${args[@]}"
echo "▸ api on the restored database"
"${compose[@]}" up -d api --wait
info="$(curl -fsS "http://127.0.0.1:$port/api/v1/server/info")"
echo "$info" | head -c 300; echo
if [ -n "$email" ]; then
  echo "▸ login $email"
  code="$(curl -sS -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$port/api/v1/auth/login/start" \
    -H 'content-type: application/json' -d "{\"email\":\"$email\"}")"
  [ "$code" = "200" ] || { echo "login start returned $code"; exit 1; }
fi
echo "✔ restore drill passed"
