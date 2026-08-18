#!/bin/sh
set -eu

workspace_root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
fixture_dir=$(mktemp -d "${TMPDIR:-/tmp}/ensub-verify-test.XXXXXX")
trap 'rm -rf "$fixture_dir"' EXIT HUP INT TERM

mkdir -p "$fixture_dir/bin"

cat >"$fixture_dir/bin/cargo" <<'EOF'
#!/bin/sh
if [ "${1:-}" = tree ]; then
  echo "synthetic cargo tree failure" >&2
  exit 19
fi
exit 0
EOF

cat >"$fixture_dir/bin/wasm-pack" <<'EOF'
#!/bin/sh
exit 0
EOF

chmod +x "$fixture_dir/bin/cargo" "$fixture_dir/bin/wasm-pack"

if PATH="$fixture_dir/bin:$PATH" sh "$workspace_root/scripts/verify.sh" wasm \
  >"$fixture_dir/stdout" 2>"$fixture_dir/stderr"; then
  echo "verify.sh wasm ignored a failing cargo tree command" >&2
  exit 1
fi

grep -q "synthetic cargo tree failure" "$fixture_dir/stderr"
echo "verify.sh propagates cargo tree failures"
