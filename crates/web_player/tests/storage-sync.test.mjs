import { describe, expect, test } from "bun:test";

import { createStorageSynchronizer } from "../js/storage-sync.js";

function createChannelPair() {
  const listeners = [[], []];
  return listeners.map((_, index) => ({
    addEventListener(type, listener) {
      if (type === "message") listeners[index].push(listener);
    },
    postMessage(data) {
      queueMicrotask(() => {
        for (const listener of listeners[1 - index]) listener({ data });
      });
    },
  }));
}

function createStorage(values = {}) {
  const entries = new Map(Object.entries(values));
  return {
    getItem(key) {
      return entries.get(key) ?? null;
    },
    removeItem(key) {
      entries.delete(key);
    },
    setItem(key, value) {
      entries.set(key, value);
    },
  };
}

describe("createStorageSynchronizer", () => {
  test("hands exact active and backup snapshots to a peer before publish resolves", async () => {
    const [firstChannel, secondChannel] = createChannelPair();
    const firstStorage = createStorage({ active: "{exact active}", backup: "{exact backup}" });
    const secondStorage = createStorage();
    const first = createStorageSynchronizer({
      activeKey: "active",
      backupKey: "backup",
      channel: firstChannel,
      storage: firstStorage,
    });
    createStorageSynchronizer({
      activeKey: "active",
      backupKey: "backup",
      channel: secondChannel,
      storage: secondStorage,
    });
    await new Promise((resolve) => setTimeout(resolve, 0));

    await first.publish();

    expect(secondStorage.getItem("active")).toBe("{exact active}");
    expect(secondStorage.getItem("backup")).toBe("{exact backup}");
  });

  test("propagates removal of both snapshot keys", async () => {
    const [firstChannel, secondChannel] = createChannelPair();
    const firstStorage = createStorage();
    const secondStorage = createStorage({ active: "old", backup: "old backup" });
    const first = createStorageSynchronizer({
      activeKey: "active",
      backupKey: "backup",
      channel: firstChannel,
      storage: firstStorage,
    });
    createStorageSynchronizer({
      activeKey: "active",
      backupKey: "backup",
      channel: secondChannel,
      storage: secondStorage,
    });
    await new Promise((resolve) => setTimeout(resolve, 0));

    await first.publish();

    expect(secondStorage.getItem("active")).toBeNull();
    expect(secondStorage.getItem("backup")).toBeNull();
  });
});
