import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { cp, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const workspaceRoot = dirname(dirname(root));
const pkg = join(root, "pkg");
const dist = join(root, "dist");
const sandboxAssets = join(workspaceRoot, "crates/web_sandbox/assets");
const run = promisify(execFile);

async function filesUnder(path) {
  const files = [];
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const child = join(path, entry.name);
    if (entry.isDirectory()) files.push(...await filesUnder(child));
    else files.push(child);
  }
  return files;
}

await rm(pkg, { recursive: true, force: true });
await rm(dist, { recursive: true, force: true });
await run("wasm-pack", [
  "build", join(workspaceRoot, "crates/wasm_bridge"), "--target", "web", "--out-dir", pkg,
  "--release", "--locked",
], {
  cwd: workspaceRoot,
  env: { ...process.env, WASM_PACK_CACHE: process.env.WASM_PACK_CACHE ?? join(workspaceRoot, "target/wasm-pack-cache") },
});

const { stdout: themeCss } = await run(
  "cargo", ["run", "--locked", "--quiet", "-p", "ensub-theme", "--bin", "ensub-theme-css"],
  { cwd: workspaceRoot },
);
if (!themeCss.trim()) throw new Error("ensub-theme-css produced an empty stylesheet");

await mkdir(join(dist, "assets"), { recursive: true });
for (const item of ["index.html", "styles.css", "manifest.webmanifest", "js", "assets", "pkg"]) {
  await cp(join(root, item), join(dist, item), { recursive: true });
}
for (const file of ["lexicon-v1.manifest.json", "lexicon-v1.postcard.gz"]) {
  await cp(join(sandboxAssets, file), join(dist, "assets", file));
}
await writeFile(join(dist, "theme.css"), themeCss, "utf8");

const outputFiles = (await filesUnder(dist)).map((path) => relative(dist, path)).sort();
const hash = createHash("sha256");
for (const file of outputFiles) {
  hash.update(file);
  hash.update(await readFile(join(dist, file)));
}
const buildId = hash.digest("hex").slice(0, 16);
const template = await readFile(join(root, "service-worker.template.js"), "utf8");
const precache = JSON.stringify(outputFiles.map((file) => `./${file}`), null, 2);
await writeFile(
  join(dist, "service-worker.js"),
  template.replaceAll("__BUILD_ID__", buildId).replace("__PRECACHE__", precache),
  "utf8",
);
console.log(`Built Ensub Player ${buildId} at ${dist}`);
