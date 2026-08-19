# Getting Started

This guide installs Ensub from a source checkout and walks through one complete
capture and review cycle.

## Prerequisites

All Rust crates use edition 2021 and declare Rust `1.93` as the minimum toolchain
for workspace applications.

Install:

- Rust and Cargo, preferably through `rustup`.
- Git, if you are obtaining the repository from source control.
- A terminal with interactive input for `esb review` and interactive
  `esb parse` selection.

The native SQLite driver is built with its bundled SQLite feature, and the
offline English lexicon is embedded in the native storage crate. A system
SQLite installation and network dictionary service are not required.

The optional surfaces have additional requirements:

- TUI: a terminal supported by `crossterm`.
- COSMIC GUI and applet: Linux graphics, Wayland/X11, and COSMIC development
  dependencies required by the pinned `libcosmic` revision.
- Ensub Player: Bun 1.3.14, wasm-pack 0.15.0, Playwright 1.62.1, Chromium,
  Firefox for the WASM browser gate, and the `wasm32-unknown-unknown` Rust
  target.
- Ensub Core sandbox: Bun 1.3.14, wasm-pack 0.15.0, Playwright 1.62.1,
  Chromium, Firefox, and the `wasm32-unknown-unknown` Rust target.
- Ensub Context: Bun 1.3.14 and configured Supabase services.

Confirm the Rust toolchain:

```bash
rustc --version
cargo --version
```

## Install the CLI and TUI

From the repository root:

```bash
cargo install --path crates/cli --locked
```

This installs the `esb` binary in Cargo's binary directory, normally
`$HOME/.cargo/bin`. Make sure that directory is on `PATH`.

For development, commands can be run without installing:

```bash
cargo run -p ensub-cli -- --help
cargo run -p ensub-cli -- stats
```

## Complete a First Review

Capture a dictionary-backed word:

```bash
esb add immersion \
  --context "Immersion turns ordinary reading into deliberate practice." \
  --source "getting-started"
```

`--source` requires `--context`. If both are omitted, Ensub still captures the
word and creates its initial review state.

An initial card is due at its capture time:

```bash
esb due
esb review --limit 1
```

The review command shows the lemma and context, waits for Enter before
revealing the pronunciation and definition, then asks for a recall rating from
0 through 5. The resulting interval is persisted immediately.

Inspect the collection summary:

```bash
esb stats
```

## Parse Text from Standard Input

Use interactive selection when stderr is connected to a terminal:

```bash
cat article.txt | esb parse --source "article.txt"
```

Use `--yes` in scripts or non-interactive environments:

```bash
printf '%s\n' "Deliberate reading reinforces unfamiliar vocabulary." \
  | esb parse --yes --source "scripted-example"
```

By default Ensub excludes common stopwords and returns at most 100 candidates.
Use `--include-stopwords` or `--max-candidates <N>` when a different parse is
needed.

## Open the TUI Reader

Open Markdown or plain text directly:

```bash
esb tui article.md
```

Run `esb tui` without a path to start in the file-path prompt. In Reader mode,
move with `h`, `j`, `k`, and `l`; press `c` to capture the focused word and `r`
to open quick review. The complete key map is in the
[User Guide](user-guide.md#tui-reader).

## Build the COSMIC Desktop Surfaces

Build all installable release binaries:

```bash
cargo build --release -p ensub-cli -p ensub-gui -p ensub-applet
```

Launch the GUI directly during development:

```bash
cargo run -p ensub-gui --bin ensub-gui
```

Launch the compact clipboard capture HUD instead of the full application:

```bash
cargo run -p ensub-gui --bin ensub-gui -- --capture
```

Install the release binaries and COSMIC integration files for the current
user:

```bash
PREFIX="$HOME/.local" sh packaging/install.sh
```

The installer copies `esb`, the COSMIC GUI and applet, integration metadata,
icons, project licenses, and lexicon notices. Ensure `$HOME/.local/bin` is on
`PATH`, then add Ensub from COSMIC's panel settings. A system-wide installation
uses the default `/usr/local` prefix and normally requires elevated filesystem
permissions.

## Build the Player Workspace

```bash
cd crates/web_player
bun install --frozen-lockfile
bun run test
bun run build
bun run verify:dist
bun run test:browser
bun run serve
```

Open `http://127.0.0.1:4175`. The player workspace is the initial screen; there
is no landing page. When the workspace contains no feeds, choose **Load Demo
Episode** to import `assets/demo-fixture.json`, its embedded transcript cues,
and the synthetic two-minute `assets/demo.mp3`. The action is available only
for an empty workspace. Fixture validation and the all-or-nothing cache update
run in Rust through `ensub-wasm`.

The build generates a content-addressed service worker containing the complete
application shell, WASM runtime, demo files, and both versioned lexicon
sidecars. After that worker finishes installing, reloads and the bundled demo
remain available offline. Use the browser's install action to add the Player as
a standalone PWA. A remote podcast feed and its enclosures must still allow
direct browser access; the Player has no proxy.

The browser gate suite requires Chromium, and the WASM browser gate uses
Firefox. WebKit is not a v0.2.0 release gate.

## Build the Offline Core Sandbox

```bash
cd crates/web_sandbox
bun install --frozen-lockfile
bun test
bun run build
bun run verify:dist
bun run serve
```

Open `http://127.0.0.1:4174`. After the service worker controls the page, the
parser, capture, review, statistics, and reload workflow remains available
offline. The build contains no Ensub Context, Supabase, LLM, or remote endpoint
reference.

## Build Ensub Context

Configure Supabase as described in the
[Ensub Context README](../crates/web_site/README.md), then build and serve the
static site:

```bash
cd crates/web_site
bun test
bun run build
bun run verify:dist
bun run serve
```

Open `http://127.0.0.1:4173`. The build output is written to
`crates/web_site/dist` and is intentionally ignored by Git.

## Create Local Release Archives

```bash
sh packaging/build-release.sh
```

The command creates CLI/TUI and COSMIC archives plus `SHA256SUMS` under
`target/release-artifacts/v0.2.0`. It does not publish them.

## Next Steps

- Learn every command and keybinding in the [User Guide](user-guide.md).
- Review native and browser persistence in [Data and Privacy](data-and-privacy.md).
- Read [Development](development.md) before changing the workspace.
