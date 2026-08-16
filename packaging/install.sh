#!/bin/sh
set -eu

prefix=${PREFIX:-/usr/local}
destdir=${DESTDIR:-}
bindir="${destdir}${prefix}/bin"
datadir="${destdir}${prefix}/share"

install -d "$bindir" "$datadir/applications" "$datadir/metainfo" \
  "$datadir/icons/hicolor/scalable/apps" "$datadir/icons/hicolor/symbolic/apps"
install -m 0755 target/release/ensub-gui "$bindir/ensub-gui"
install -m 0755 target/release/ensub-applet "$bindir/ensub-applet"
install -m 0644 packaging/dev.ensub.Ensub.desktop "$datadir/applications/"
install -m 0644 packaging/dev.ensub.Ensub.Applet.desktop "$datadir/applications/"
install -m 0644 packaging/dev.ensub.Ensub.metainfo.xml "$datadir/metainfo/"
install -m 0644 packaging/icons/dev.ensub.Ensub.svg \
  "$datadir/icons/hicolor/scalable/apps/"
install -m 0644 packaging/icons/dev.ensub.Ensub-symbolic.svg \
  "$datadir/icons/hicolor/symbolic/apps/"
