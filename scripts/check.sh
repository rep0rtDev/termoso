#!/usr/bin/env bash
# Runs the same checks as .github/workflows/ci.yml, locally.
#
#   scripts/check.sh                 all areas whose toolchain is present
#   scripts/check.sh rust web        only these areas
#
# Areas: rust web desktop android deploy. Areas whose tools are missing are
# skipped with a note; a failing check aborts (set KEEP_GOING=1 to run all).
set -uo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

areas=("$@")
[ ${#areas[@]} -eq 0 ] && areas=(rust web desktop android deploy)

failed=()
# run <area> <dir> <command...> — runs the command inside <dir>.
run() {
  local area=$1 dir=$2
  shift 2
  printf '\n\033[1;34m▶ %s (%s):\033[0m %s\n' "$area" "$dir" "$*"
  if ! (cd "$root/$dir" && "$@"); then
    failed+=("$area: $*")
    [ -n "${KEEP_GOING:-}" ] || finish
  fi
}
skip() { printf '\n\033[1;33m⏭ %s:\033[0m %s\n' "$1" "$2"; }
finish() {
  echo
  if [ ${#failed[@]} -eq 0 ]; then
    printf '\033[1;32mAll checks passed.\033[0m\n'
    exit 0
  fi
  printf '\033[1;31mFailed:\033[0m\n'
  printf '  %s\n' "${failed[@]}"
  exit 1
}
has() { command -v "$1" >/dev/null 2>&1; }

check_rust() {
  has cargo || { skip rust "cargo not found"; return; }
  run rust . cargo fmt --all -- --check
  run rust . cargo clippy --workspace --all-targets --locked
  # Server tests use PostgreSQL/Redis/MinIO/Mailpit from deploy/docker-compose.dev.yml
  # when reachable and skip otherwise (TERMOSO_TEST_REQUIRE_SERVICES=1 to insist).
  run rust . cargo test --workspace --locked
  # Advisories, licence allow-list, banned crates and registry sources: deny.toml.
  if cargo deny --version >/dev/null 2>&1; then
    run rust . cargo deny --locked check advisories bans licenses sources
  else
    skip rust "cargo-deny not found (cargo install cargo-deny --locked); CI runs it"
  fi
}

npm_checks() {
  local area=$1 dir=$2
  shift 2
  has npm || { skip "$area" "npm not found"; return; }
  run "$area" "$dir" npm ci --no-audit --no-fund
  run "$area" "$dir" npm audit --audit-level=moderate
  local script
  for script in "$@"; do run "$area" "$dir" npm run "$script"; done
}

check_web() {
  if ! has wasm-pack; then
    skip web "wasm-pack not found (cargo install wasm-pack)"
    return
  fi
  npm_checks web web wasm format:check typecheck lint build
}

check_desktop() {
  npm_checks desktop apps/desktop format:check typecheck lint test build
}

check_android() {
  if [ -z "${ANDROID_HOME:-}" ] || [ ! -d "${ANDROID_HOME}" ]; then
    skip android "ANDROID_HOME not set"
    return
  fi
  has cargo-ndk || { skip android "cargo-ndk not found (cargo install cargo-ndk)"; return; }
  run android apps/android ./gradlew --no-daemon :app:assembleDebug :app:testDebugUnitTest :app:lintDebug
}

check_deploy() {
  local compose=()
  if has docker && docker compose version >/dev/null 2>&1; then
    compose=(docker compose)
  elif has podman-compose; then
    compose=(podman-compose)
  fi
  if [ ${#compose[@]} -gt 0 ]; then
    run deploy deploy env TERMOSO_ENV_FILE=.env.example POSTGRES_PASSWORD=x MINIO_ROOT_USER=x MINIO_ROOT_PASSWORD=x \
      "${compose[@]}" -f docker-compose.yml --env-file .env.example --profile bridge --profile proxy config -q
    run deploy deploy "${compose[@]}" -f docker-compose.dev.yml config -q
  else
    skip deploy "docker compose / podman-compose not found"
  fi
  local quadlet
  for quadlet in /usr/libexec/podman/quadlet /usr/lib/podman/quadlet; do
    if [ -x "$quadlet" ]; then
      run deploy . env QUADLET_UNIT_DIRS="$root/deploy/quadlet" "$quadlet" -dryrun -no-kmsg-log
      return
    fi
  done
  skip deploy "quadlet binary not found (part of podman)"
}

for area in "${areas[@]}"; do
  case $area in
    rust) check_rust ;;
    web) check_web ;;
    desktop) check_desktop ;;
    android) check_android ;;
    deploy) check_deploy ;;
    *) echo "unknown area: $area (rust web desktop android deploy)" >&2; exit 2 ;;
  esac
done
finish
