#!/bin/sh
set -eu

workspace_root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$workspace_root"

target=${RELEASE_TARGET:-x86_64-unknown-linux-gnu}
version=$(cargo metadata --no-deps --format-version 1 --locked \
  | jq -r '.packages[] | select(.name == "ensub-cli") | .version')
if [ -z "$version" ] || [ "$version" = "null" ]; then
  echo "could not resolve the ensub-cli package version" >&2
  exit 1
fi

artifact_dir="$workspace_root/target/release-artifacts/v$version"
binary_dir="$workspace_root/target/$target/release"
stage_dir=$(mktemp -d "${TMPDIR:-/tmp}/ensub-release-stage.XXXXXX")
trap 'rm -rf "$stage_dir"' EXIT HUP INT TERM

cargo build --release --locked --target "$target" \
  -p ensub-cli -p ensub-gui -p ensub-applet

DESTDIR="$stage_dir" PREFIX=/usr BINARY_DIR="$binary_dir" sh packaging/install.sh
desktop-file-validate "$stage_dir/usr/share/applications/dev.ensub.Ensub.desktop"
desktop-file-validate "$stage_dir/usr/share/applications/dev.ensub.Ensub.Applet.desktop"
appstreamcli validate --no-net "$stage_dir/usr/share/metainfo/dev.ensub.Ensub.metainfo.xml"
sh packaging/smoke-test.sh "$stage_dir"

mkdir -p "$artifact_dir"
cli_name="ensub-cli-$version-$target.tar.gz"
cosmic_name="ensub-cosmic-$version-$target.tar.gz"
cli_archive="$artifact_dir/$cli_name"
cosmic_archive="$artifact_dir/$cosmic_name"
checksums="$artifact_dir/SHA256SUMS"
rm -f "$cli_archive" "$cosmic_archive" "$checksums"

tar --sort=name --owner=0 --group=0 --numeric-owner --mtime=@0 \
  -C "$stage_dir" -cf - -T packaging/cli-files.txt | gzip -n >"$cli_archive"
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime=@0 \
  -C "$stage_dir" -cf - -T packaging/cosmic-files.txt | gzip -n >"$cosmic_archive"
(
  cd "$artifact_dir"
  sha256sum "$cli_name" "$cosmic_name" >SHA256SUMS
  sha256sum --check SHA256SUMS
)

echo "Local release artifacts: $artifact_dir"
