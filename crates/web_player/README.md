# Ensub Player

`web_player` is Ensub's installable, local-first podcast workspace. It is a
static HTML/CSS/ES-module application backed by the `ensub-wasm` player cache.
It remains separate from `web_sandbox`, the stable v0.1 core learning harness.

## Development

```sh
bun install --frozen-lockfile
bun test tests/*.test.mjs
bun run build
bun run verify:dist
bun run test:browser
```

`bun run serve` serves the built artifact on `http://127.0.0.1:4175`.

The build compiles `ensub-wasm`, generates CSS from `ensub-theme`, copies the
versioned lexicon sidecars from `web_sandbox`, and generates a content-addressed
service worker. Generated packages, distributions, dependencies, and test
output are ignored.

## Storage And Fetching

The player stores one opaque Rust snapshot at `snapshot-v1` in the
`ensub-player` IndexedDB database. Writes are serialized with the
`ensub.player.workspace.v1` Web Lock and announced through a same-named
`BroadcastChannel`. Playback position, rate, and volume are session state and
are not included in the snapshot.

Feed and transcript requests are made directly by the browser. URLs must be
credential-free HTTP(S), requests time out after 15 seconds, redirects are
revalidated, and streamed response limits are enforced. Servers must permit
browser access; the player does not route requests through a proxy.
