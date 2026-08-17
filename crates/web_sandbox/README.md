# Ensub Core Sandbox

This package is the offline reference harness for Ensub Core. It bundles the
`ensub-wasm` module and the generated browser lexicon, stores a versioned
snapshot in `localStorage`, and coordinates mutations with Web Locks.

It intentionally has no dependency on Ensub Context, Supabase, an LLM, or any
remote endpoint. Browsers without Web Locks can parse and inspect existing
state but open the WASM adapter read-only.

```bash
bun test
bun run build
bun run verify:dist
bun run serve
```

Browser integration tests run with `bun run test:browser` after the build.
