# Data and Privacy

Ensub Core is local-first: native surfaces persist to SQLite and the offline
browser sandbox persists a versioned snapshot to `localStorage`. The separate,
optional Ensub Context companion uses an anonymous Supabase identity,
owner-scoped Postgres records, and an OpenAI-compatible model. These products
do not synchronize with each other and Ensub does not include telemetry.

## Native Database

CLI, TUI, COSMIC GUI, and COSMIC applet resolve the same platform-standard
database path by default:

```text
<local data directory>/ensub/ensub.sqlite3
```

On a typical Linux installation this is:

```text
$XDG_DATA_HOME/ensub/ensub.sqlite3
```

or, when `XDG_DATA_HOME` is unset:

```text
$HOME/.local/share/ensub/ensub.sqlite3
```

The parent directory and schema are created automatically on first open.
Current schema records include:

- `words`: surface term, lemma, IPA, definitions, and creation time;
- `contexts`: sentence, source label, capture time, and word relationship;
- `review_state`: ease, repetitions, interval, due time, and last rating;
- `review_events`: immutable before/after history for committed reviews.

## Native Path Overrides

The CLI and TUI accept either a global option or environment variable:

```bash
esb --database /path/to/ensub.sqlite3 stats
ENSUB_DATABASE_PATH=/path/to/ensub.sqlite3 esb review
```

The COSMIC GUI currently uses a separate environment variable name:

```bash
ensub-gui --database /path/to/ensub.sqlite3
ENSUB_DATABASE=/path/to/ensub.sqlite3 ensub-gui
```

The applet currently always resolves the default platform path. Therefore,
use the default database when the applet must share data with the CLI and GUI.

Path overrides are especially useful for tests, demos, and isolated
collections. They do not migrate or copy data from the default database.

## Concurrency and Integrity

Every native connection enables:

- SQLite foreign keys;
- WAL journal mode;
- `NORMAL` synchronous mode;
- a five-second busy timeout.

Capture batches and committed reviews use immediate transactions. Review
updates compare the state originally shown to the user with the stored state;
if another process changed it first, Ensub returns a conflict instead of
overwriting it. This allows CLI, TUI, GUI, and applet processes to use the same
database safely under normal local workloads.

Do not edit the database manually while Ensub is running. The schema is
versioned and migrated automatically by the storage adapter.

## Native Lexicon Cache

The compressed lexicon is compiled into the native storage crate. On first
dictionary-backed use, Ensub validates and extracts it to:

```text
<cache directory>/ensub/lexicon/
```

On typical Linux systems this is `$XDG_CACHE_HOME/ensub/lexicon` or
`$HOME/.cache/ensub/lexicon`. Cache deletion does not delete vocabulary or
review history; Ensub recreates the verified lexicon from its embedded asset.

## Ensub Core Browser Storage

The offline sandbox stores its versioned JSON snapshot under
`ensub.sandbox.v1`. Capture, review, and reset mutations are serialized with
the browser's Web Locks API. If Web Locks are unavailable, parsing and state
queries remain available but the sandbox opens read-only.

The lexicon, WASM module, application shell, and service worker are bundled in
one same-origin static distribution. Its build rejects remote endpoint,
Supabase, LLM, and credential references. Once installed by the service
worker, the complete workflow can reload offline. Reset removes only this
snapshot; corrupt or newer snapshots are preserved until an explicit reset.

## Ensub Player Storage

The player persists podcast feeds, episode metadata, selected transcript
resources, and parsed transcript cues in one opaque Rust snapshot in the
browser's `ensub-player` IndexedDB database. Audio is streamed and is not added
to this snapshot. Playback position, rate, and volume are not persisted.

An explicit transcript-token capture stores its normalized lemma, sentence,
feed and episode identity, publication metadata when available, enclosure and
transcript URLs, cue IDs, selected token span, capture playback position, host
timestamp, and padded logical audio range in the local learning snapshot under
`ensub.sandbox.v1`. The range references the original enclosure; Ensub does not
copy, record, or transcode audio. Opening a lookup does not write data, and no
lookup or capture automatically contacts an LLM or dictionary service.

Feed and transcript requests connect directly from the browser to the resource
host. Browser CORS and network policy apply. The player has no implicit proxy
and does not send feed URLs, query strings, transcript contents, or cache
contents to an Ensub service. The installed application shell, WASM module,
icons, demo media, and versioned lexicon sidecars are available offline; remote
audio and transcripts must already be cached or reachable.

Learning snapshot v1 data migrates forward in memory. Ensub writes schema v2
only after a complete successful mutation; corrupt, newer, or failed-migration
snapshots remain unchanged so reset or future recovery remains possible.

## Ensub Context Storage

Ensub Context persists an anonymous Supabase session in browser storage. Saved
phrases, sentences, optional surrounding context, and generated lexical fields
are stored in `vocabulary_records` with the anonymous user's ID. Row Level
Security allows each authenticated session to select and insert only its own
rows.

Clearing browser site data loses access to that anonymous identity. Removing
the anonymous user from Supabase cascades to its records. Ensub Context
requires a network connection and does not import the Core WASM snapshot.

## Backup and Restore

Ensub does not yet provide a built-in export, backup, or restore command.

For a consistent native backup, either:

1. Close all Ensub native surfaces before copying `ensub.sqlite3`; or
2. Use a SQLite-aware backup tool while the database is open.

Do not copy only the main database file while Ensub is running in WAL mode,
because recently committed data may still be present in its `-wal` file.

To restore, close all native surfaces and replace the database with a valid
backup at the same path. Preserve the original until the restored database has
opened successfully. Ensub refuses schemas newer than the running build
supports.

Ensub Context currently has no export/import UI. The Supabase project
operator remains responsible for database backup and retention.

## Resetting Data

- Native: there is no reset command. Back up the database, close all Ensub
  processes, then remove the specific database only when permanent deletion is
  intended.
- Core sandbox: use its explicit reset control to delete `ensub.sandbox.v1`.
- Ensub Context: no reset/delete UI is provided. Removing an
  anonymous user from Supabase deletes that user's vocabulary records.

These stores never synchronize automatically. Resetting one does not affect
the other.
