# Local Linux Packaging

Ensub v0.2.0 releases are packaged locally. The scripts in this
directory do not push, publish, upload, tag, or create a hosted release.

Build the native binaries, stage a `/usr` installation, validate the desktop
metadata, run the synthetic CLI review cycle, and create both deterministic
archives:

```bash
sh packaging/build-release.sh
```

Output is written under `target/release-artifacts/v<version>/`:

- `ensub-cli-<version>-x86_64-unknown-linux-gnu.tar.gz` contains `esb`, whose
  `tui` subcommand provides the terminal application.
- `ensub-cosmic-<version>-x86_64-unknown-linux-gnu.tar.gz` contains the GUI,
  applet, desktop entries, AppStream metadata, and icons.
- `SHA256SUMS` covers both archives.

Both archives include project licenses, the lexicon provenance record, and
all bundled lexicon third-party notices. GNU tar normalizes ordering,
ownership, and modification time; `gzip -n` removes timestamp/name headers.

To inspect the installer without writing outside a temporary directory:

```bash
stage="$(mktemp -d)"
DESTDIR="$stage" PREFIX=/usr sh packaging/install.sh
find "$stage" -type f -print | sort
```
