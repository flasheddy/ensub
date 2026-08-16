# Data and Privacy

Ensub's native surfaces are local-first and persist to SQLite. The optional
contextual web assistant uses an anonymous Supabase identity, owner-scoped
Postgres records, and an OpenAI-compatible model. Ensub does not synchronize
native vocabulary with the web app and does not include telemetry.

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

## Contextual Web App Storage

The web app persists an anonymous Supabase session in browser storage. Saved
phrases, sentences, optional surrounding context, and generated lexical fields
are stored in `vocabulary_records` with the anonymous user's ID. Row Level
Security allows each authenticated session to select and insert only its own
rows.

Clearing browser site data loses access to that anonymous identity. Removing
the anonymous user from Supabase cascades to its records. The web app requires
a network connection and does not use the old offline service-worker or WASM
snapshot store.

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

The contextual web app currently has no export/import UI. The Supabase project
operator remains responsible for database backup and retention.

## Resetting Data

- Native: there is no reset command. Back up the database, close all Ensub
  processes, then remove the specific database only when permanent deletion is
  intended.
- Web app: no reset/delete UI is provided. Removing an
  anonymous user from Supabase deletes that user's vocabulary records.

These stores never synchronize automatically. Resetting one does not affect
the other.
