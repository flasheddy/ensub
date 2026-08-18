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

Transcript text is rendered from Rust-produced cue-relative UTF-16 token spans.
Selecting a token performs an offline WASM lookup; a separate Capture action
persists the bounded cross-cue sentence, structured podcast provenance, logical
audio slice, lexical data, and initial review state atomically.

Due cards open in a modal review session inside the player. Prompt DTOs contain
only saved contexts and logical audio slices; lemma and definition data are
requested from WASM only after Reveal. The host temporarily checkpoints the
episode source, position, play/pause state, rate, volume, and mute state while a
snippet runner seeks and stops the same audio element. Exiting review restores
that checkpoint. Missing remote audio never blocks text reveal or rating.

Ratings use the shared Rust SM-2 implementation. Each prompt carries an
`rs1` SHA-256 token derived from canonical current `ReviewState` content. Reveal
and rating reload that state, and rating also uses storage compare-and-swap, so
another tab's update produces `review_conflict` and a queue refresh instead of
an automatic retry.

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

Learning captures share the existing `ensub.sandbox.v1` localStorage snapshot
with the Core harness and use the `ensub.sandbox.storage.v1` Web Lock. Snapshot
schema v1 is migrated in memory and becomes v2 only after a successful write.
The player cache remains a separate IndexedDB envelope.

## Optional Context Provider

Contextual explanation is an explicit lookup-panel action and is never called
by import, playback, token selection, capture, or review. The default
`openai_chat_completions` adapter accepts a user-configured HTTPS endpoint
(HTTP is limited to loopback development), model, and credential. Metadata is
versioned in local storage. Credentials use session storage by default and are
written to local storage only when **Remember on this device** is selected.

Before the first request for an adapter/endpoint/disclosure-version tuple, the
player displays the exact payload: selected word, saved sentence, candidate
local senses, and minimal episode label. It never includes the full transcript
or audio. The Rust language engine creates the prompts and candidate IDs, the
request asks for `response_format: { "type": "json_object" }`, and Rust
validates the response against the static schema before it is rendered. AI
output remains visually separate and ephemeral; it never overwrites bundled
lexicon data or captured definitions.
