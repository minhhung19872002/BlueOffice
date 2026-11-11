#!/bin/bash
# Full regression of the BlueOffice checkout this script is run from (repo root = cwd).
# Each step writes <name>.log and appends "<name> exit=<code>" to summary.txt in $BO_REGRESS_DIR
# (default: $TEMP/bo-regress). Run it in the background; read summary.txt when it prints DONE.
# Never start a second copy while one is running (two runs make tests time out).
set -u
export PATH="$USERPROFILE/.cargo/bin:$PATH" CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
L="${BO_REGRESS_DIR:-$TEMP/bo-regress}"
mkdir -p "$L"
: > "$L/summary.txt"
step() {
  name=$1
  shift
  "$@" > "$L/$name.log" 2>&1
  echo "$name exit=$?" >> "$L/summary.txt"
}
step fmt cargo fmt --all -- --check
step clippy cargo clippy --workspace --all-targets -- -D warnings
step cargotest cargo test --workspace --no-fail-fast
step deny cargo deny check licenses bans sources
step wasm pnpm build:wasm
step typecheck pnpm typecheck
step lint pnpm lint
step license pnpm license-check
step pnpmtest pnpm test
step dotnet dotnet test BlueOffice.sln
step dotnetfmt dotnet format BlueOffice.sln --verify-no-changes
step webbuild pnpm --filter @blueoffice/web build
(cd tests/e2e && step e2e pnpm exec playwright test --workers=1)
step collab pnpm --filter @blueoffice/e2e test:collab
echo DONE >> "$L/summary.txt"
