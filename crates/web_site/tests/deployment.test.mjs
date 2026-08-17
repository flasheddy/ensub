import test from "node:test";
import assert from "node:assert/strict";
import { access, mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { REQUIRED_FILES, verifyDist } from "../scripts/verify-dist.mjs";

const repositoryRoot = new URL("../../../", import.meta.url);

test("the web package pins Bun and runs project scripts with Bun", async () => {
  const packageJson = JSON.parse(
    await readFile(new URL("crates/web_site/package.json", repositoryRoot), "utf8"),
  );

  assert.equal(packageJson.packageManager, "bun@1.3.14");
  assert.equal(packageJson.scripts.test, "bun test tests/*.test.mjs");
  assert.equal(packageJson.scripts.build, "bun scripts/build.mjs");
  assert.equal(packageJson.scripts["verify:dist"], "bun scripts/verify-dist.mjs");
  assert.equal(packageJson.scripts.serve, "bun scripts/serve.mjs");
});

test("GitHub Pages builds with Bun and publishes only the web dist artifact", async () => {
  const workflow = await readFile(
    new URL(".github/workflows/deploy-web-site.yml", repositoryRoot),
    "utf8",
  );

  assert.match(workflow, /bun-version:\s*1\.3\.14/);
  assert.match(workflow, /dtolnay\/rust-toolchain@1\.93/);
  assert.match(workflow, /- crates\/theme\/\*\*/);
  assert.match(workflow, /- Cargo\.toml/);
  assert.match(workflow, /- Cargo\.lock/);
  assert.match(workflow, /working-directory:\s*crates\/web_site/);
  assert.match(workflow, /run:\s*bun install --frozen-lockfile/);
  assert.match(workflow, /run:\s*bun test/);
  assert.match(workflow, /run:\s*bun run build/);
  assert.match(workflow, /run:\s*bun run verify:dist/);
  assert.match(workflow, /path:\s*crates\/web_site\/dist/);
  assert.doesNotMatch(workflow, /path:\s*\.\s*$/m);
});

test("the web package has no Vercel deployment configuration", async () => {
  await assert.rejects(
    access(new URL("../vercel.json", import.meta.url)),
    (error) => error?.code === "ENOENT",
  );
});

test("the static build copies only the contextual app deployment files", async () => {
  const source = await readFile(new URL("../scripts/build.mjs", import.meta.url), "utf8");

  assert.doesNotMatch(source, /vercel/i);
  assert.match(source, /"index\.html"/);
  assert.match(source, /"styles\.css"/);
  assert.match(source, /"js"/);
  assert.match(source, /"service-worker\.js"/);
  assert.doesNotMatch(source, /wasm-pack|"pkg"|"assets"/);
  assert.match(source, /cargo/);
  assert.match(source, /ensub-theme-css/);
  assert.match(source, /theme\.css/);
  assert.match(source, /--locked/);
});

test("the page loads generated semantic theme variables before component styles", async () => {
  const source = await readFile(new URL("../index.html", import.meta.url), "utf8");
  const themeLink = source.indexOf('href="./theme.css"');
  const stylesLink = source.indexOf('href="./styles.css"');

  assert.notEqual(themeLink, -1);
  assert.ok(themeLink < stylesLink);
  assert.match(source, /<meta name="theme-color" content="#1e1e2e">/);
});

test("web presentation uses semantic theme variables and state attributes", async () => {
  const index = await readFile(new URL("../index.html", import.meta.url), "utf8");
  const styles = await readFile(new URL("../styles.css", import.meta.url), "utf8");
  const view = await readFile(new URL("../js/vocabulary-view.js", import.meta.url), "utf8");
  const indexWithoutThemeFallback = index.replace("#1e1e2e", "");

  assert.doesNotMatch(indexWithoutThemeFallback, /#[0-9a-f]{3,8}/i);
  assert.doesNotMatch(styles, /#[0-9a-f]{3,8}|rgba?\(/i);
  assert.doesNotMatch(view, /#[0-9a-f]{3,8}|rgba?\(/i);
  assert.match(styles, /var\(--ensub-color-background\)/);
  assert.match(styles, /var\(--ensub-color-accent\)/);
  assert.match(view, /dataset\.status/);
  assert.match(view, /dataset\.kind/);
});

test("the page pins browser dependencies and excludes old runtime entrypoints", async () => {
  const source = await readFile(new URL("../index.html", import.meta.url), "utf8");

  assert.match(source, /@tailwindcss\/browser@4\.1\.12/);
  assert.match(source, /@supabase\/supabase-js@2\.57\.4/);
  assert.doesNotMatch(source, /wasm|checkins\.js|runtime\.js/);
});

test("the retirement worker clears legacy Ensub caches without precaching", async () => {
  const source = await readFile(new URL("../service-worker.js", import.meta.url), "utf8");

  assert.doesNotMatch(source, /PRECACHE|cache\.add|cache\.put/);
  assert.match(source, /key\.startsWith\("ensub-"\)/);
  assert.match(source, /registration\.unregister/);
});

test("browser configuration contains no privileged or LLM secrets", async () => {
  const source = await readFile(new URL("../js/supabase-config.js", import.meta.url), "utf8");

  assert.match(source, /publishableKey/);
  assert.doesNotMatch(source, /service[_-]?role|sb_secret_|LLM_API_KEY|OPENAI_API_KEY|ZHIPU_API_KEY/i);
});

test("the bootstrap error state names the unavailable session", async () => {
  const source = await readFile(new URL("../js/vocabulary-view.js", import.meta.url), "utf8");

  assert.match(source, /Session unavailable/);
});

test("the async submit handler retains the form before awaiting", async () => {
  const source = await readFile(new URL("../js/app.js", import.meta.url), "utf8");

  assert.match(source, /const form = event\.currentTarget;/);
  assert.match(source, /form\.reset\(\)/);
  assert.doesNotMatch(source, /event\.currentTarget\.reset\(\)/);
});

test("the bundle verifier identifies the referring file and missing asset", async () => {
  const root = await mkdtemp(join(tmpdir(), "ensub-dist-verifier-"));

  try {
    await mkdir(join(root, "js"));
    await writeFile(join(root, "index.html"), '<script type="module" src="./js/missing.js"></script>');

    await assert.rejects(
      verifyDist(root, { requiredFiles: ["index.html"] }),
      /index\.html references missing local file js\/missing\.js/,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("the deployment verifier requires generated theme CSS", () => {
  assert.ok(REQUIRED_FILES.includes("theme.css"));
});
