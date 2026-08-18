# Changelog

All notable changes to Ensub are documented in this file.

## Unreleased

## 0.2.0 - 2026-08-18

- Add the installable Ensub Player baseline with direct podcast feed import,
  synchronized VTT/SRT transcripts, local transcript caching, and offline PWA
  shell and lexicon assets.
- Add offline transcript-token lookup, bounded cross-cue sentence capture,
  structured podcast provenance, non-destructive padded audio slices, and
  in-player SM-2 review with graceful text fallback when audio is unavailable.
- Add atomic multi-context lemma captures and recoverable browser snapshot
  migration from schema v1 to v2, including exact-byte preservation after
  failed writes and a local Player data export/reset recovery dialog.
- Add optional, explicit contextual disambiguation with per-endpoint consent,
  minimal payload disclosure, schema validation, and no automatic provider
  calls during import, playback, lookup, capture, or review.
- Add cross-origin CORS/no-CORS browser fixtures, keyboard and focus coverage,
  375px-through-desktop responsive gates, WASM dependency enforcement, and
  production panic/placeholder and generated-artifact hardening checks.

## 0.1.0-rc1 - 2026-08-17

- Converge native CLI, TUI, COSMIC GUI, and applet workflows on the shared
  local SQLite engine and bundled offline lexicon.
- Add the isolated Ensub Core browser sandbox with real WASM parsing,
  capture, SRS review, versioned snapshots, Web Locks coordination, and an
  offline service-worker cache.
- Keep Ensub Context as a separate optional online Supabase/LLM companion.
- Preserve compare-and-swap conflicts through the COSMIC applet reducer.
- Add deterministic local Linux release archives, staged smoke tests,
  metadata validation, and SHA-256 manifests.

Releases are packaged locally only; this repository does not publish artifacts
automatically.
