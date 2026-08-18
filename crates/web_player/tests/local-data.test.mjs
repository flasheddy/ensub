import { expect, test } from "bun:test";
import { LOCAL_EXPORT_FORMAT, createLocalDataManager } from "../js/local-data.js";

class MemoryStorage {
  values = new Map();
  getItem(key) { return this.values.get(key) ?? null; }
  setItem(key, value) { this.values.set(key, String(value)); }
  removeItem(key) { this.values.delete(key); }
}

test("local export preserves exact learning text and Player bytes without provider data", async () => {
  const local = new MemoryStorage();
  const exactLearning = '{ "format": "ensub-browser-storage", "schemaVersion": 2 }';
  local.setItem("ensub.sandbox.v1", exactLearning);
  local.setItem("ensub.disambiguation.credential.local.v1", "synthetic-secret");
  const manager = createLocalDataManager({
    store: { load: async () => new Uint8Array([0, 127, 128, 255]) },
    local,
    settings: { clear() {} },
  });

  const exported = await manager.exportValue();

  expect(exported).toEqual({
    format: LOCAL_EXPORT_FORMAT,
    schemaVersion: 1,
    learningSnapshot: exactLearning,
    playerCacheBase64: "AH+A/w==",
  });
  expect(JSON.stringify(exported)).not.toContain("synthetic-secret");
});

test("reset clears only Player learning and provider state", async () => {
  const local = new MemoryStorage();
  local.setItem("ensub.sandbox.v1", "learning");
  local.setItem("unrelated", "preserved");
  const calls = [];
  const manager = createLocalDataManager({
    store: { clear: async () => calls.push("player") },
    local,
    settings: { clear: () => calls.push("provider") },
  });

  await manager.reset();

  expect(calls).toEqual(["player", "provider"]);
  expect(local.getItem("ensub.sandbox.v1")).toBeNull();
  expect(local.getItem("unrelated")).toBe("preserved");
});
