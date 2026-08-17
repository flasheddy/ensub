import { access, readdir, readFile, stat } from "node:fs/promises";
import { dirname, extname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const REQUIRED_FILES = [
  "index.html",
  "theme.css",
  "styles.css",
  "service-worker.js",
  "assets/lexicon-v1.manifest.json",
  "assets/lexicon-v1.postcard.gz",
  "js/app.js",
  "js/assets.js",
  "js/coordinator.js",
  "js/controller.js",
  "js/sandbox-client.js",
  "js/view.js",
  "pkg/ensub_wasm.js",
  "pkg/ensub_wasm_bg.wasm",
];

const TEXT_EXTENSIONS = new Set([".css", ".html", ".js", ".json", ".mjs"]);

function isExternalReference(reference) {
  return /^(?:[a-z]+:|\/\/|#)/i.test(reference);
}

function cleanReference(reference) {
  return reference.split(/[?#]/, 1)[0];
}

async function listFiles(root) {
  const files = [];
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) files.push(...await listFiles(path));
    else files.push(path);
  }
  return files;
}

async function assertReferenceExists(dist, referringFile, reference) {
  if (!reference || isExternalReference(reference)) return false;
  const target = resolve(dirname(join(dist, referringFile)), cleanReference(reference));
  const relativeTarget = relative(dist, target);
  if (relativeTarget === ".." || relativeTarget.startsWith(`..${sep}`)) {
    throw new Error(`${referringFile} references a file outside dist: ${reference}`);
  }
  try {
    await stat(target);
  } catch {
    throw new Error(`${referringFile} references missing local file ${relativeTarget}`);
  }
  return true;
}

function references(source) {
  return [
    ...source.matchAll(/\b(?:src|href)=["']([^"']+)["']/g),
    ...source.matchAll(/(?:\bfrom\s*|\bimport\s*\()\s*["']([^"']+)["']/g),
  ].map((match) => match[1]);
}

function assertNoRemoteEndpoint(file, source) {
  const matches = source.match(/(?:https?:)?\/\/[a-z0-9.-]+(?::\d+)?/gi) ?? [];
  if (matches.length > 0) {
    throw new Error(`${file} contains a cross-origin endpoint: ${matches[0]}`);
  }
  if (/\b(?:supabase|openai|anthropic|api[_-]?key|service[_-]?role)\b/i.test(source)) {
    throw new Error(`${file} contains a forbidden cloud or credential reference`);
  }
}

function precacheEntries(worker) {
  const match = worker.match(/const PRECACHE = (\[[\s\S]*?\]);/);
  if (!match) throw new Error("service worker does not define a literal precache list");
  return JSON.parse(match[1]);
}

export async function verifyDist(dist, { requiredFiles = REQUIRED_FILES } = {}) {
  const resolvedDist = resolve(dist);
  for (const file of requiredFiles) {
    try {
      await access(join(resolvedDist, file));
    } catch {
      throw new Error(`Missing required sandbox file: ${file}`);
    }
  }

  const files = await listFiles(resolvedDist);
  let referencesChecked = 0;
  for (const file of files.filter((path) => TEXT_EXTENSIONS.has(extname(path)))) {
    const source = await readFile(file, "utf8");
    const referringFile = relative(resolvedDist, file);
    assertNoRemoteEndpoint(referringFile, source);
    for (const reference of references(source)) {
      if (await assertReferenceExists(resolvedDist, referringFile, reference)) referencesChecked += 1;
    }
  }

  const worker = await readFile(join(resolvedDist, "service-worker.js"), "utf8");
  const cached = new Set(precacheEntries(worker).map((entry) => entry.replace(/^\.\//, "")));
  for (const file of files.map((path) => relative(resolvedDist, path))) {
    if (file !== "service-worker.js" && !cached.has(file)) {
      throw new Error(`service worker precache is missing ${file}`);
    }
  }

  return { filesChecked: files.length, referencesChecked };
}

const scriptPath = fileURLToPath(import.meta.url);
if (process.argv[1] && resolve(process.argv[1]) === scriptPath) {
  const root = dirname(dirname(scriptPath));
  try {
    const result = await verifyDist(join(root, "dist"));
    console.log(`Verified offline sandbox: ${result.filesChecked} files, ${result.referencesChecked} local references.`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
