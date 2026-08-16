import { access, readdir, readFile, stat } from "node:fs/promises";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const REQUIRED_FILES = [
  "index.html",
  "styles.css",
  "service-worker.js",
  "vercel.json",
  "js/app.js",
  "js/supabase-api.js",
  "js/supabase-config.js",
  "js/vocabulary-model.js",
  "js/vocabulary-view.js",
];

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

  const cleaned = cleanReference(reference);
  const referringPath = join(dist, referringFile);
  const target = resolve(dirname(referringPath), cleaned);
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

function htmlReferences(source) {
  return [...source.matchAll(/\b(?:src|href)=["']([^"']+)["']/g)].map((match) => match[1]);
}

function moduleReferences(source) {
  return [...source.matchAll(/(?:\bfrom\s*|\bimport\s*\()\s*["']([^"']+)["']/g)]
    .map((match) => match[1])
    .filter((reference) => reference.startsWith("."));
}

export async function verifyDist(dist, { requiredFiles = REQUIRED_FILES } = {}) {
  const resolvedDist = resolve(dist);
  for (const file of requiredFiles) {
    try {
      await access(join(resolvedDist, file));
    } catch {
      throw new Error(`Missing required deployment file: ${file}`);
    }
  }

  let referencesChecked = 0;
  const indexSource = await readFile(join(resolvedDist, "index.html"), "utf8");
  for (const reference of htmlReferences(indexSource)) {
    if (await assertReferenceExists(resolvedDist, "index.html", reference)) referencesChecked += 1;
  }

  const files = await listFiles(resolvedDist);
  for (const file of files.filter((path) => path.endsWith(".js"))) {
    const source = await readFile(file, "utf8");
    const referringFile = relative(resolvedDist, file);
    for (const reference of moduleReferences(source)) {
      if (await assertReferenceExists(resolvedDist, referringFile, reference)) referencesChecked += 1;
    }
  }

  return { filesChecked: files.length, referencesChecked };
}

const scriptPath = fileURLToPath(import.meta.url);
if (process.argv[1] && resolve(process.argv[1]) === scriptPath) {
  const root = dirname(dirname(scriptPath));
  try {
    const result = await verifyDist(join(root, "dist"));
    console.log(`Verified dist: ${result.filesChecked} files and ${result.referencesChecked} local references.`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
