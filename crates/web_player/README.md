# Ensub Player

`web_player` is Ensub's installable, local-first podcast workspace. It is a
static HTML/CSS/ES-module application backed by the `ensub-wasm` player cache.
The workspace itself is rendered on initial load rather than being hidden
behind a landing page. It remains separate from `web_sandbox`, the stable v0.1
core learning harness.

## Development

```sh
bun install --frozen-lockfile
bun test tests/*.test.mjs
bun run build
bun run verify:dist
bun run test:browser
```

`bun run serve` serves the built artifact on `http://127.0.0.1:4175`. Build
before starting that preview. The local server uses the `audio/mpeg` content
type and supports MP3 byte-range responses so seeking can be exercised locally.

The build compiles `ensub-wasm`, generates CSS from `ensub-theme`, copies the
complete `assets` tree and the versioned lexicon sidecars from `web_sandbox`,
and generates a content-addressed service worker. `bun run verify:dist` requires
the demo fixture, demo MP3, `assets/lexicon-v1.manifest.json`, and compressed
`assets/lexicon-v1.postcard.gz` sidecar, then verifies that every distribution
file is present in the generated precache. Generated packages, distributions,
dependencies, and test output are ignored.

## Thin Host Boundary

When the Rust workspace contains no feeds, the initial workspace shows **Load
Demo Episode**. The host fetches at most 1 MiB from
`assets/demo-fixture.json` and passes those bytes unchanged to
`EnsubPlayerWorkspace.importDemoFixture`. The schema-v1 fixture points to the
synthetic two-minute `assets/demo.mp3` and contains source-ordered, pre-parsed
WebVTT cue data, including an overlap and a gap. Rust resolves relative HTTP(S)
URLs, validates metadata, media duration, cue ordering and bounds, tokenizes cue
text, derives stable identities, and atomically imports the feed, episode, and
transcript. Import requires an empty feed collection; failure leaves the live
workspace and persisted bytes unchanged. The demo action is hidden after any
feed exists and becomes usable again after a failed empty-workspace import.

Rust also owns cue synchronization and adjacent-cue policy. The DOM audio
host rounds `timeupdate` and `seeked` timestamps to milliseconds and passes them
directly to `syncAt`; animation frames provide additional samples during
playback. The resulting DTO supplies all active cue indices, the anchor cue,
and the preceding cue. `nextCueAt` and `previousCueAt` determine shortcut
targets across overlaps, gaps, transcript boundaries, and times outside the
transcript.

JavaScript owns only browser effects: the DOM audio element, bounded fetches,
IndexedDB and Web Locks, rendered highlight and ARIA state, scrolling, focus,
and user-initiated seeks. All simultaneously active cues are highlighted.
Following centers the Rust-selected anchor smoothly, or without animation when
reduced motion is requested. Wheel, touch, or manual reader scrolling suspends
following and exposes **Return to active cue**; following resumes only when that
control is chosen. Clicking a cue row or its timestamp outside a token seeks to
that cue's `startMs`. Selecting transcript text does not seek.

Transcript text is rendered from Rust-produced cue-relative UTF-16 token spans.
Selecting a token performs an offline WASM lookup; a separate Capture action
persists the bounded cross-cue sentence, structured podcast provenance, logical
audio slice, lexical data, and initial review state atomically.

## Keyboard Commands

Player shortcuts are global while the workspace canvas or a transcript token
has focus:

| Key | Action |
|---|---|
| `Space` | Play or pause |
| `J` / Down Arrow | Seek to the next cue selected by Rust |
| `K` / Up Arrow | Seek to the previous cue selected by Rust |
| Left Arrow / Right Arrow | Skip backward or forward 5 seconds |
| `[` / `]` | Step speed through 0.75x, 1x, 1.25x, 1.5x, 1.75x, and 2x |
| `R` | Open or close Review |

Native `Tab` and `Shift+Tab` order includes every transcript token. Press
`Enter` on a focused token to open its offline lookup. Player shortcuts are
ignored when a form field, select, ordinary button, link, or editable region
has focus, when Alt/Ctrl/Meta/Shift is held, or while a non-review dialog is
open. Only `R` remains active inside the Review dialog so it can close the
session. Repeated `Space` and `R` keydown events are ignored.

## Review

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

## Offline Installation

Service-worker registration starts before WASM initialization. The generated
worker precaches the application shell, WASM package, both lexicon sidecars,
`assets/demo-fixture.json`, and the two-minute `assets/demo.mp3`. Once the worker
has installed, the bundled demo and previously cached workspace data can reload
offline. Remote feeds, transcripts, and enclosures remain available only when
they have been cached or their origins are reachable and permit browser access.

## Storage And Fetching

The player stores one opaque Rust snapshot at `snapshot-v1` in the
`ensub-player` IndexedDB database. Writes are serialized with the
`ensub.player.workspace.v1` Web Lock and announced through a same-named
`BroadcastChannel`. Playback position, rate, and volume are session state and
are not included in the snapshot. IndexedDB write promises resolve only after
the transaction commits, so an aborted transaction cannot replace the live
workspace.

Feed and transcript requests are made directly by the browser. URLs must be
credential-free HTTP(S), requests time out after 15 seconds, redirects are
revalidated, and streamed response limits are enforced. Servers must permit
browser access; the player does not route requests through a proxy.

Learning captures share the existing `ensub.sandbox.v1` localStorage snapshot
with the Core harness and use the `ensub.sandbox.storage.v1` Web Lock. Snapshot
schema v1 is migrated in memory and becomes v2 only after a successful write.
The player cache remains a separate IndexedDB envelope.

The **Local data** dialog remains available when normal Player boot fails. Its
export contains the exact learning snapshot text and base64-encoded Player
cache bytes in the versioned `ensub-local-export` format. Provider settings and
credentials are excluded. Reset removes only these two Ensub snapshots plus
Ensub provider configuration, credentials, and consent; unrelated origin data,
service-worker assets, and native SQLite are preserved. Import is not part of
the v0.2.0 baseline.

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
