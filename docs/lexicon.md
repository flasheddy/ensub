# Offline Lexicon

Ensub performs dictionary lookup without remote APIs. Native and browser
surfaces use two generated representations of the same pinned source data.

## Sources and Transformations

| Source | Version | Ensub use | License |
|---|---|---|---|
| Open English WordNet | 2025 | Lemmas, parts of speech, and definitions | CC BY 4.0, with inherited Princeton WordNet notice |
| CMU Pronouncing Dictionary | 0.7b distributed by `cmudict-fast` 0.8.0 | Pronunciations | CMUdict permissive license |

The generator:

- removes entries that do not have both a definition and pronunciation;
- retains up to three ranked definitions;
- derives deterministic inflected-form mappings;
- converts CMUdict ARPAbet to broad General American IPA;
- packages native lookup tables as SQLite compressed with Zstandard;
- exports a compact Postcard/Gzip representation for browsers.

The current native artifact contains:

| Record | Count |
|---|---:|
| Lexemes | 32,463 |
| Surface forms | 49,207 |
| Ranked senses | 78,463 |

Exact source and generated artifact SHA-256 values are recorded in
[`PROVENANCE.toml`](../crates/sqlite_storage/assets/PROVENANCE.toml). Full
third-party notices are under
[`THIRD_PARTY_LICENSES`](../crates/sqlite_storage/assets/THIRD_PARTY_LICENSES).

## Native Artifact

`crates/sqlite_storage/assets/lexicon-v1.sqlite3.zst` is included in the
`ensub-sqlite` build. Dictionary-backed commands validate and extract it into
the platform cache on first use. Extraction uses a temporary file and atomic
rename; subsequent opens reuse the installed artifact after verifying its
digest.

The extracted database is read-only application data, not the user's
vocabulary database. Deleting the cache causes Ensub to extract it again and
does not affect captures or reviews.

## Browser Artifact

`crates/web_sandbox/assets/lexicon-v1.postcard.gz` is loaded by the WebAssembly
sandbox. Its manifest records schema version, entry counts, source labels,
compressed size, and digest:

[`crates/web_sandbox/assets/lexicon-v1.manifest.json`](../crates/web_sandbox/assets/lexicon-v1.manifest.json)

The static service worker includes the asset in its application cache. The
browser lexicon implements the same portable `Lexicon` contract as the native
SQLite lookup.

## Lookup Behavior

`language_engine` normalizes a surface word and produces deterministic lemma
candidates before lexicon lookup. Successful entries provide:

- a dictionary lemma;
- broad IPA pronunciation;
- one or more part-of-speech and definition pairs.

Words that miss the bundled dictionary are counted by parse operations but
are not captured as dictionary-backed cards. The GUI Reader can still focus
such a token and reports that it is not in the bundled lexicon.

## Regeneration

Normal development and release builds use committed assets and require no
network access. Regeneration is a maintainer operation that requires local,
explicit source corpus files.

Generate the native database and compressed artifact:

```bash
cargo run -p ensub-lexicon-builder -- \
  <oewn.xml.gz> \
  <cmudict.dict> \
  <output.sqlite3> \
  <output.sqlite3.zst>
```

Export the browser artifact:

```bash
cargo run -p ensub-lexicon-builder -- \
  export-browser \
  <input.sqlite3.zst> \
  <output.postcard.gz>
```

Before replacing committed artifacts, verify:

1. Source versions and input checksums.
2. Generated native and compressed checksums.
3. Schema version and corpus counts.
4. Native bundled-lexicon tests.
5. Browser asset generation and WASM tests.
6. Attribution and third-party license files.

The lexicon tests pin schema and corpus metadata so unintended source or
generation changes fail visibly.
