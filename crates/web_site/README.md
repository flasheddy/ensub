# Ensub Contextual Vocabulary Web App

`crates/web_site` is a static, single-page contextual English assistant. A
learner submits a target word or phrase, the sentence where it appeared, and
optional surrounding context. An authenticated Supabase Edge Function calls an
OpenAI-compatible model, validates the structured lexical analysis, and saves
the complete encounter to the learner's private history.

The page uses vanilla HTML and JavaScript, Tailwind CSS through a pinned CDN
runtime, and the pinned Supabase JavaScript SDK. No LLM credential is shipped
to the browser.

## Supabase Setup

1. Enable anonymous sign-ins under **Authentication > Providers > Anonymous**.
2. Apply [`supabase/vocabulary_records.sql`](supabase/vocabulary_records.sql).
   The migration intentionally stops if `vocabulary_records` contains rows,
   because ownership for legacy records cannot be inferred safely.
3. Deploy `supabase/functions/analyze-vocabulary` with JWT verification enabled.
4. Configure these Edge Function secrets in the Supabase Dashboard:

   ```text
   LLM_BASE_URL=https://open.bigmodel.cn/api/paas/v4
   LLM_MODEL=glm-4-flash
   LLM_API_KEY=<provider credential>
   ALLOWED_ORIGINS=https://your-site.example
   ```

   `LLM_BASE_URL` may point to another OpenAI-compatible Chat Completions API.
   `ALLOWED_ORIGINS` accepts a comma-separated list. Local preview origins on
   ports `4173` are allowed by the function source.

5. Put only the public project URL and publishable key in
   [`js/supabase-config.js`](js/supabase-config.js). Publishable keys are
   browser-visible identifiers and are safe only with the committed RLS and
   least-privilege grants. Never place an LLM key, Supabase secret key,
   database password, or privileged key in frontend files.

The schema grants authenticated sessions only `SELECT` and `INSERT`, and both
operations require `auth.uid() = user_id`. Anonymous Supabase sessions use the
`authenticated` database role and therefore receive the same owner isolation
as conventional signed-in users.

## Build and Preview

The site has no generated WASM dependency:

```bash
cd crates/web_site
bun install --frozen-lockfile
bun test
bun run build
bun run verify:dist
bun run serve
```

Open `http://127.0.0.1:4173`. The provider-neutral static artifact is written
to `dist/`, which is intentionally ignored by Git.

Bun 1.3.14 is pinned in `package.json`. This package has no registry
dependencies, so Bun intentionally does not retain an empty lockfile.

The deployment includes a non-caching retirement service worker. Its only job
is to remove caches left by the previous offline WASM site and unregister
itself, preventing stale users from remaining on the retired interface.

## Deploy to GitHub Pages

`.github/workflows/deploy-web-site.yml` builds this directory on relevant
pushes to `main` and uploads only `crates/web_site/dist` to GitHub Pages. It
can also be run manually from GitHub Actions.

The project site is `https://flasheddy.github.io/ensub/`. Supabase must allow
the origin `https://flasheddy.github.io`; the `/ensub/` path is not part of a
CORS origin. Configure the Edge Function with:

```text
ALLOWED_ORIGINS=https://flasheddy.github.io
```

## Deploy to Vercel as a Fallback

Build locally or in CI and deploy the generated directory:

```bash
cd crates/web_site
bun test
bun run build
bun run verify:dist
bunx vercel deploy dist
bunx vercel deploy dist --prod
```

Add the production URL to the `ALLOWED_ORIGINS` Edge Function secret before
testing the deployed app. `vercel.json` keeps the public Supabase browser
configuration revalidated so project key rotations are not held in cache.

## Privacy and Operational Limits

Vocabulary records are private to the anonymous Supabase user persisted in the
browser. Clearing browser site data loses access to that identity. Deleting the
anonymous user cascades to its vocabulary records.

The implementation uses session-only abuse protection. It prevents
unauthenticated function calls and duplicate in-flight browser submissions,
but it does not add CAPTCHA or an application-level quota ledger. A public
deployment should monitor provider usage and apply provider-side limits.
