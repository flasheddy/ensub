#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <staged-root>" >&2
  exit 2
fi

staged_root=$1
esb="$staged_root/usr/bin/esb"
if [ ! -x "$esb" ]; then
  echo "staged esb binary is missing: $esb" >&2
  exit 1
fi

smoke_root=$(mktemp -d "${TMPDIR:-/tmp}/ensub-release-smoke.XXXXXX")
trap 'rm -rf "$smoke_root"' EXIT HUP INT TERM
database="$smoke_root/ensub.sqlite3"
cache="$smoke_root/cache"
mkdir -p "$cache"

ENSUB_DATABASE_PATH="$database" XDG_CACHE_HOME="$cache" \
  "$esb" add immersion \
  --context "Immersion makes a synthetic release test repeatable." \
  --source "release-smoke" >/dev/null

due_before=$(ENSUB_DATABASE_PATH="$database" XDG_CACHE_HOME="$cache" "$esb" due)
if [ "$due_before" != "1" ]; then
  echo "expected one due card after capture, got: $due_before" >&2
  exit 1
fi

review_command="env ENSUB_DATABASE_PATH='$database' XDG_CACHE_HOME='$cache' '$esb' review --limit 1"
printf '\n\n' | script -qefc "$review_command" /dev/null >/dev/null

due_after=$(ENSUB_DATABASE_PATH="$database" XDG_CACHE_HOME="$cache" "$esb" due)
stats=$(ENSUB_DATABASE_PATH="$database" XDG_CACHE_HOME="$cache" "$esb" stats | tr -d '\r')
if [ "$due_after" != "0" ]; then
  echo "expected no due cards after rating 4, got: $due_after" >&2
  exit 1
fi
printf '%s\n' "$stats" | grep -F "total: 1" >/dev/null
printf '%s\n' "$stats" | grep -F "due: 0" >/dev/null
printf '%s\n' "$stats" | grep -F "1-6d: 1" >/dev/null

echo "Release smoke passed: add -> due -> review -> stats"
