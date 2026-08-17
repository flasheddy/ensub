import { expect, test } from "bun:test";
import { REQUIRED_FILES } from "../scripts/verify-dist.mjs";

test("PWA artifact contract includes player, media, WASM, icons, and lexicon sidecars", () => {
  expect(REQUIRED_FILES).toEqual(expect.arrayContaining([
    "index.html", "manifest.webmanifest", "service-worker.js", "theme.css",
    "assets/icons/icon-192.png", "assets/icons/icon-512.png", "assets/demo/cover.png",
    "assets/demo/audio.wav", "assets/demo/feed.xml", "assets/demo/transcript.vtt",
    "assets/lexicon-v1.manifest.json", "assets/lexicon-v1.postcard.gz",
    "pkg/ensub_wasm.js", "pkg/ensub_wasm_bg.wasm", "js/app.js",
  ]));
});
