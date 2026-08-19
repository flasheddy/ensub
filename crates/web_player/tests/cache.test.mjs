import { expect, test } from "bun:test";
import { createIndexedDbStore, createWorkspaceCoordinator } from "../js/cache.js";

function controlledIndexedDb() {
  let transaction;
  const indexedDB = {
    open() {
      const request = {};
      queueMicrotask(() => {
        request.result = {
          objectStoreNames: { contains: () => true },
          transaction(_name, mode) {
            const tx = { mode, error: null };
            tx.objectStore = () => ({
              get() {
                const operation = {};
                queueMicrotask(() => { operation.result = new Uint8Array([1]).buffer; operation.onsuccess?.(); });
                return operation;
              },
              put() {
                const operation = {};
                queueMicrotask(() => { operation.result = undefined; operation.onsuccess?.(); });
                return operation;
              },
              delete(key) {
                const operation = {};
                operation.key = key;
                queueMicrotask(() => { operation.result = undefined; operation.onsuccess?.(); });
                return operation;
              },
            });
            transaction = tx;
            return tx;
          },
        };
        request.onsuccess?.();
      });
      return request;
    },
  };
  return { indexedDB, transaction: () => transaction };
}

test("IndexedDB writes resolve only after transaction commit", async () => {
  const fixture = controlledIndexedDb();
  const store = createIndexedDbStore(fixture.indexedDB);
  let resolved = false;
  const saving = store.save(new Uint8Array([2])).then(() => { resolved = true; });
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(resolved).toBe(false);
  fixture.transaction().oncomplete();
  await saving;
  expect(resolved).toBe(true);
});

test("IndexedDB abort after request success rejects the write", async () => {
  const fixture = controlledIndexedDb();
  const store = createIndexedDbStore(fixture.indexedDB);
  const saving = store.save(new Uint8Array([2]));
  await new Promise((resolve) => setTimeout(resolve, 0));
  fixture.transaction().error = new Error("synthetic commit failure");
  fixture.transaction().onabort();
  await expect(saving).rejects.toThrow("synthetic commit failure");
});

test("Player store exports exact bytes and clears only its snapshot key", async () => {
  const fixture = controlledIndexedDb();
  const store = createIndexedDbStore(fixture.indexedDB);
  const loading = store.load();
  await new Promise((resolve) => setTimeout(resolve, 0));
  fixture.transaction().oncomplete();
  expect(await loading).toEqual(new Uint8Array([1]));
  const clearing = store.clear();
  await new Promise((resolve) => setTimeout(resolve, 0));
  fixture.transaction().oncomplete();
  await clearing;
});

test("workspace mutations load latest inside the lock and replace live state only after write", async () => {
  const events = [];
  let stored = new Uint8Array([1]);
  const handles = [];
  const coordinator = createWorkspaceCoordinator({
    locks: { request: async (_name, _options, operation) => { events.push("lock"); return operation(); } },
    store: {
      load: async () => { events.push("load"); return stored; },
      save: async (snapshot) => { events.push("save"); stored = snapshot; },
    },
    workspaceFactory(snapshot) {
      const handle = {
        source: snapshot[0] ?? 0,
        importFeed() { events.push("mutate"); return { revision: 2 }; },
        snapshot() { return new Uint8Array([2]); },
      };
      handles.push(handle);
      return handle;
    },
  });
  await coordinator.open();
  events.length = 0;
  await coordinator.mutate("importFeed", "url", new Uint8Array(), 1);
  expect(events).toEqual(["lock", "load", "mutate", "save"]);
  expect(coordinator.workspace).toBe(handles.at(-1));
  expect(coordinator.workspace.source).toBe(1);
});
