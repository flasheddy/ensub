#!/bin/sh
set -eu

prefix=${PREFIX:-/usr/local}
destdir=${DESTDIR:-}
binary_dir=${BINARY_DIR:-target/release}
bindir="${destdir}${prefix}/bin"
datadir="${destdir}${prefix}/share"
docdir="$datadir/doc/ensub"

install -d "$bindir" "$datadir/applications" "$datadir/metainfo" \
  "$datadir/icons/hicolor/scalable/apps" "$datadir/icons/hicolor/symbolic/apps" \
  "$docdir/THIRD_PARTY_LICENSES"
install -m 0755 "$binary_dir/esb" "$bindir/esb"
install -m 0755 "$binary_dir/ensub-gui" "$bindir/ensub-gui"
install -m 0755 "$binary_dir/ensub-applet" "$bindir/ensub-applet"
install -m 0644 packaging/dev.ensub.Ensub.desktop "$datadir/applications/"
install -m 0644 packaging/dev.ensub.Ensub.Applet.desktop "$datadir/applications/"
install -m 0644 packaging/dev.ensub.Ensub.metainfo.xml "$datadir/metainfo/"
install -m 0644 packaging/icons/dev.ensub.Ensub.svg \
  "$datadir/icons/hicolor/scalable/apps/"
install -m 0644 packaging/icons/dev.ensub.Ensub-symbolic.svg \
  "$datadir/icons/hicolor/symbolic/apps/"
install -m 0644 README.md LICENSE-APACHE LICENSE-MIT "$docdir/"
install -m 0644 crates/sqlite_storage/assets/README.md "$docdir/LEXICON.md"
install -m 0644 crates/sqlite_storage/assets/PROVENANCE.toml "$docdir/"
install -m 0644 crates/sqlite_storage/assets/THIRD_PARTY_LICENSES/CMUDICT.txt \
  crates/sqlite_storage/assets/THIRD_PARTY_LICENSES/OEWN-CC-BY-4.0.txt \
  crates/sqlite_storage/assets/THIRD_PARTY_LICENSES/PRINCETON-WORDNET-3.0.txt \
  "$docdir/THIRD_PARTY_LICENSES/"
