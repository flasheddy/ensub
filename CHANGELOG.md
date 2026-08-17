# Changelog

All notable changes to Ensub are documented in this file.

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

Release candidates are packaged locally only; this repository does not
publish v0.1.0-rc1 artifacts automatically.
