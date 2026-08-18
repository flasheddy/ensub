import { expect, test } from "bun:test";
import { cp, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { REQUIRED_FILES, verifyDist } from "../scripts/verify-dist.mjs";

const playerRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const VALID_MANIFEST = {
  name: "Ensub Player",
  short_name: "Ensub",
  start_url: "./",
  scope: "./",
  display: "standalone",
  icons: [
    { src: "./assets/icons/icon-192.png", sizes: "192x192", type: "image/png" },
    { src: "./assets/icons/icon-512.png", sizes: "512x512", type: "image/png" },
  ],
};

async function createDistFixture(manifest = VALID_MANIFEST, { icon512Source = "icon-512.png" } = {}) {
  const dist = await mkdtemp(join(tmpdir(), "ensub-player-dist-"));
  for (const file of REQUIRED_FILES) {
    await mkdir(dirname(join(dist, file)), { recursive: true });
    await writeFile(join(dist, file), "");
  }
  await cp(join(playerRoot, "assets/icons/icon-192.png"), join(dist, "assets/icons/icon-192.png"));
  if (icon512Source) {
    await cp(join(playerRoot, `assets/icons/${icon512Source}`), join(dist, "assets/icons/icon-512.png"));
  }
  await writeFile(join(dist, "manifest.webmanifest"), JSON.stringify(manifest));
  const precache = REQUIRED_FILES.filter((file) => file !== "service-worker.js").map((file) => `./${file}`);
  await writeFile(join(dist, "service-worker.js"), `const PRECACHE = ${JSON.stringify(precache)};`);
  return dist;
}

test("PWA artifact contract includes player, media, WASM, icons, and lexicon sidecars", () => {
  expect(REQUIRED_FILES).toEqual(expect.arrayContaining([
    "index.html", "manifest.webmanifest", "service-worker.js", "theme.css",
    "assets/icons/icon-192.png", "assets/icons/icon-512.png", "assets/demo/cover.png",
    "assets/demo-fixture.json", "assets/demo.mp3",
    "assets/lexicon-v1.manifest.json", "assets/lexicon-v1.postcard.gz",
    "pkg/ensub_wasm.js", "pkg/ensub_wasm_bg.wasm", "js/app.js",
    "js/disambiguation-adapter.js", "js/disambiguation-settings.js", "js/shortcuts.js", "js/snippet-runner.js",
  ]));
  expect(REQUIRED_FILES).not.toEqual(expect.arrayContaining([
    "assets/demo/audio.wav", "assets/demo/feed.xml", "assets/demo/transcript.vtt", "assets/demo/transcript.txt",
  ]));
});

test("rejects a manifest without the required standalone launch contract", async () => {
  const cases = [
    ["start_url", undefined, "start_url must be ./"],
    ["scope", "/", "scope must be ./"],
    ["display", "browser", "display must be standalone"],
  ];

  for (const [field, value, message] of cases) {
    const manifest = { ...VALID_MANIFEST, [field]: value };
    const dist = await createDistFixture(manifest);
    try {
      await expect(verifyDist(dist)).rejects.toThrow(message);
    } finally {
      await rm(dist, { recursive: true, force: true });
    }
  }
});

test("rejects manifest icons without the expected PNG declarations", async () => {
  const cases = [
    [
      { ...VALID_MANIFEST, icons: [] },
      "must declare ./assets/icons/icon-192.png",
    ],
    [
      {
        ...VALID_MANIFEST,
        icons: VALID_MANIFEST.icons.map((icon) => icon.sizes === "192x192" ? { ...icon, type: "image/jpeg" } : icon),
      },
      "./assets/icons/icon-192.png must declare sizes 192x192 and type image/png",
    ],
    [
      {
        ...VALID_MANIFEST,
        icons: VALID_MANIFEST.icons.map((icon) => icon.sizes === "512x512" ? { ...icon, sizes: "512x256" } : icon),
      },
      "./assets/icons/icon-512.png must declare sizes 512x512 and type image/png",
    ],
  ];

  for (const [manifest, message] of cases) {
    const dist = await createDistFixture(manifest);
    try {
      await expect(verifyDist(dist)).rejects.toThrow(message);
    } finally {
      await rm(dist, { recursive: true, force: true });
    }
  }
});

test("rejects icon files that are not PNGs with the declared dimensions", async () => {
  const cases = [
    [null, "assets/icons/icon-512.png is not a valid PNG with an IHDR chunk"],
    ["icon-192.png", "assets/icons/icon-512.png has PNG dimensions 192x192; expected 512x512"],
  ];

  for (const [icon512Source, message] of cases) {
    const dist = await createDistFixture(VALID_MANIFEST, { icon512Source });
    try {
      await expect(verifyDist(dist)).rejects.toThrow(message);
    } finally {
      await rm(dist, { recursive: true, force: true });
    }
  }
});
