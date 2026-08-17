#!/bin/sh
set -eu

workspace_root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$workspace_root"
mode=${1:-all}

verify_rust() {
  cargo fmt --all -- --check
  cargo check --workspace
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  cargo tree -p core_engine
  cargo tree -p language_engine
}

verify_wasm() {
  cargo check -p ensub-wasm --target wasm32-unknown-unknown
  cargo clippy -p ensub-wasm --target wasm32-unknown-unknown --all-targets -- -D warnings
  WASM_PACK_CACHE="$workspace_root/target/wasm-pack-cache" \
    wasm-pack test --firefox --headless crates/wasm_bridge
  cargo tree -p ensub-wasm --target wasm32-unknown-unknown \
    | tee "$workspace_root/target/ensub-wasm-tree.txt"
  if grep -E 'ensub-(sqlite|gui|applet|llm|tui)|libcosmic|rusqlite|reqwest|tokio' \
    "$workspace_root/target/ensub-wasm-tree.txt"; then
    echo "native adapter or UI dependency entered the WASM graph" >&2
    exit 1
  fi
}

verify_web() {
  (
    cd crates/web_player
    bun install --frozen-lockfile
    bun test tests/*.test.mjs
    bun run build
    bun run verify:dist
    bun run test:browser
  )
  (
    cd crates/web_sandbox
    bun install --frozen-lockfile
    bun test tests/*.test.mjs
    bun run build
    bun run verify:dist
    bun run test:browser
  )
  (
    cd crates/web_site
    bun test
    bun run build
    bun run verify:dist
  )
}

verify_release_smoke() (
  stage_dir=$(mktemp -d "${TMPDIR:-/tmp}/ensub-verify-stage.XXXXXX")
  trap 'rm -rf "$stage_dir"' EXIT HUP INT TERM
  cargo build --locked -p ensub-cli -p ensub-gui -p ensub-applet
  DESTDIR="$stage_dir" PREFIX=/usr BINARY_DIR="$workspace_root/target/debug" \
    sh packaging/install.sh
  desktop-file-validate "$stage_dir/usr/share/applications/dev.ensub.Ensub.desktop"
  desktop-file-validate "$stage_dir/usr/share/applications/dev.ensub.Ensub.Applet.desktop"
  appstreamcli validate --no-net "$stage_dir/usr/share/metainfo/dev.ensub.Ensub.metainfo.xml"
  sh packaging/smoke-test.sh "$stage_dir"
)

verify_secrets() {
  command -v gitleaks >/dev/null 2>&1 || {
    echo "gitleaks is required for the secrets verification mode" >&2
    exit 1
  }
  gitleaks git --redact --no-banner .
  git diff --no-ext-diff --binary | gitleaks stdin --redact --no-banner
  git diff --cached --no-ext-diff --binary | gitleaks stdin --redact --no-banner
  git ls-files --others --exclude-standard | while IFS= read -r file; do
    gitleaks dir --redact --no-banner "$file"
  done
}

case "$mode" in
  rust) verify_rust ;;
  wasm) verify_wasm ;;
  web) verify_web ;;
  release-smoke) verify_release_smoke ;;
  release) sh packaging/build-release.sh ;;
  secrets) verify_secrets ;;
  all)
    verify_rust
    verify_wasm
    verify_web
    verify_release_smoke
    verify_secrets
    sh packaging/build-release.sh
    ;;
  *)
    echo "usage: $0 {rust|wasm|web|release-smoke|release|secrets|all}" >&2
    exit 2
    ;;
esac
