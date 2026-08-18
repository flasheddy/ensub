import { access, readdir, readFile, stat } from "node:fs/promises";
import { Buffer } from "node:buffer";
import { dirname, extname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const REQUIRED_FILES = [
  "index.html", "manifest.webmanifest", "theme.css", "styles.css", "service-worker.js",
  "assets/icons/icon-192.png", "assets/icons/icon-512.png", "assets/demo/cover.png",
  "assets/demo-fixture.json", "assets/demo.mp3",
  "assets/lexicon-v1.manifest.json", "assets/lexicon-v1.postcard.gz",
  "js/app.js", "js/audio-host.js", "js/cache.js", "js/controller.js", "js/state.js",
  "js/disambiguation-adapter.js", "js/disambiguation-settings.js", "js/shortcuts.js", "js/snippet-runner.js",
  "js/transport.js", "js/view.js", "js/wasm-client.js",
  "pkg/ensub_wasm.js", "pkg/ensub_wasm_bg.wasm",
];
const TEXT_EXTENSIONS = new Set([".css", ".html", ".js", ".json", ".mjs", ".webmanifest", ".xml"]);
const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

export function findEmbeddedCredential(source) {
  const patterns = [
    ["OpenAI-style credential", /\bsk-[A-Za-z0-9_-]{32,}\b/],
    ["GitHub-style credential", /\bgh[opusr]_[A-Za-z0-9]{30,}\b/],
    ["Slack-style credential", /\bxox[baprs]-[A-Za-z0-9-]{20,}\b/],
    ["AWS access identifier", /\bAKIA[A-Z0-9]{16}\b/],
    ["literal bearer credential", /\bBearer\s+[A-Za-z0-9._~-]{24,}\b/],
  ];
  return patterns.find(([, pattern]) => pattern.test(source))?.[0] ?? null;
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
function localReferences(source) {
  return [
    ...source.matchAll(/\b(?:src|href)=['"]([^'"]+)['"]/g),
    ...source.matchAll(/(?:\bfrom\s*|\bimport\s*\()\s*['"]([^'"]+)['"]/g),
  ].map((match) => match[1]).filter((value) => !/^(?:[a-z]+:|\/\/|#)/i.test(value));
}
function precacheEntries(worker) {
  const match = worker.match(/const PRECACHE = (\[[\s\S]*?\]);/);
  if (!match) throw new Error("service worker does not define a literal precache list");
  return JSON.parse(match[1]);
}

async function readPngDimensions(path, label) {
  const image = await readFile(path);
  const hasIhdr = image.length >= 24
    && image.subarray(0, PNG_SIGNATURE.length).equals(PNG_SIGNATURE)
    && image.readUInt32BE(8) === 13
    && image.toString("ascii", 12, 16) === "IHDR";
  if (!hasIhdr) throw new Error(`${label} is not a valid PNG with an IHDR chunk`);
  const width = image.readUInt32BE(16);
  const height = image.readUInt32BE(20);
  if (width === 0 || height === 0) throw new Error(`${label} has invalid PNG dimensions ${width}x${height}`);
  return { width, height };
}

async function readManifest(root) {
  let manifest;
  try {
    manifest = JSON.parse(await readFile(join(root, "manifest.webmanifest"), "utf8"));
  } catch (error) {
    throw new Error(`manifest.webmanifest is not valid JSON: ${error.message}`);
  }
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    throw new Error("manifest.webmanifest must contain a JSON object");
  }
  for (const [field, expected] of [
    ["start_url", "./"],
    ["scope", "./"],
    ["display", "standalone"],
  ]) {
    if (manifest[field] !== expected) throw new Error(`manifest.webmanifest ${field} must be ${expected}`);
  }
  const icons = Array.isArray(manifest.icons) ? manifest.icons : [];
  for (const [src, sizes] of [
    ["./assets/icons/icon-192.png", "192x192"],
    ["./assets/icons/icon-512.png", "512x512"],
  ]) {
    const declarations = icons.filter((icon) => icon && typeof icon === "object" && icon.src === src);
    if (declarations.length === 0) throw new Error(`manifest.webmanifest must declare ${src}`);
    if (!declarations.some((icon) => icon.sizes === sizes && icon.type === "image/png")) {
      throw new Error(`manifest.webmanifest ${src} must declare sizes ${sizes} and type image/png`);
    }
    const label = src.replace(/^\.\//, "");
    const { width, height } = await readPngDimensions(join(root, label), label);
    if (`${width}x${height}` !== sizes) {
      throw new Error(`${label} has PNG dimensions ${width}x${height}; expected ${sizes}`);
    }
  }
  return manifest;
}

export async function verifyDist(dist) {
  const root = resolve(dist);
  for (const file of REQUIRED_FILES) await access(join(root, file));
  await readManifest(root);
  const files = await listFiles(root);
  for (const file of files.filter((path) => TEXT_EXTENSIONS.has(extname(path)))) {
    const source = await readFile(file, "utf8");
    const referring = relative(root, file);
    const credential = findEmbeddedCredential(source);
    if (credential) throw new Error(`${referring} contains a high-confidence ${credential}`);
    for (const reference of localReferences(source)) {
      const target = resolve(dirname(file), reference.split(/[?#]/, 1)[0]);
      const child = relative(root, target);
      if (child === ".." || child.startsWith(`..${sep}`)) throw new Error(`${referring} references outside dist`);
      await stat(target).catch(() => { throw new Error(`${referring} references missing file ${child}`); });
    }
  }
  const cached = new Set(precacheEntries(await readFile(join(root, "service-worker.js"), "utf8")).map((entry) => entry.replace(/^\.\//, "")));
  for (const file of [
    "assets/demo-fixture.json", "assets/demo.mp3",
    "assets/lexicon-v1.manifest.json", "assets/lexicon-v1.postcard.gz",
  ]) {
    if (!cached.has(file)) throw new Error(`service worker precache is missing required asset ${file}`);
  }
  for (const file of files.map((path) => relative(root, path))) {
    if (file !== "service-worker.js" && !cached.has(file)) throw new Error(`service worker precache is missing ${file}`);
  }
  return { filesChecked: files.length };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = dirname(dirname(fileURLToPath(import.meta.url)));
  verifyDist(join(root, "dist"))
    .then(({ filesChecked }) => console.log(`Verified offline player: ${filesChecked} files.`))
    .catch((error) => { console.error(error.message); process.exitCode = 1; });
}
