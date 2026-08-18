import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";

test("cross-tab learning writes refresh the due count", async () => {
  const source = await readFile(new URL("../js/app.js", import.meta.url), "utf8");
  expect(source).toContain("LEARNING_STORAGE_KEY");
  expect(source).toContain('addEventListener("storage"');
  expect(source).toContain("controller.refreshDueCount()");
});

test("service worker registration does not depend on successful WASM boot", async () => {
  const source = await readFile(new URL("../js/app.js", import.meta.url), "utf8");
  expect(source.indexOf("navigator.serviceWorker.register")).toBeGreaterThan(-1);
  expect(source.indexOf("navigator.serviceWorker.register")).toBeLessThan(source.indexOf("await initializeWasm()"));
});
