export const DATABASE_NAME = "ensub-player";
export const STORE_NAME = "workspace";
export const SNAPSHOT_KEY = "snapshot-v1";
export const LOCK_NAME = "ensub.player.workspace.v1";
export const CHANNEL_NAME = LOCK_NAME;

export function createIndexedDbStore(indexedDB = globalThis.indexedDB) {
  let databasePromise;
  function database() {
    databasePromise ??= new Promise((resolve, reject) => {
      const request = indexedDB.open(DATABASE_NAME, 1);
      request.onupgradeneeded = () => {
        if (!request.result.objectStoreNames.contains(STORE_NAME)) request.result.createObjectStore(STORE_NAME);
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    return databasePromise;
  }
  async function transaction(mode, operation) {
    const db = await database();
    return new Promise((resolve, reject) => {
      const tx = db.transaction(STORE_NAME, mode);
      const request = operation(tx.objectStore(STORE_NAME));
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
      tx.onabort = () => reject(tx.error);
    });
  }
  return {
    async load() {
      const value = await transaction("readonly", (store) => store.get(SNAPSHOT_KEY));
      return value ? new Uint8Array(value) : new Uint8Array();
    },
    async save(snapshot) {
      await transaction("readwrite", (store) => store.put(snapshot.slice().buffer, SNAPSHOT_KEY));
    },
  };
}

export function createWorkspaceCoordinator({
  locks = globalThis.navigator?.locks,
  store = createIndexedDbStore(),
  workspaceFactory,
  channel = typeof BroadcastChannel === "function" ? new BroadcastChannel(CHANNEL_NAME) : null,
}) {
  let workspace = null;
  if (typeof locks?.request !== "function") {
    throw new Error("Web Locks are required for writable player storage.");
  }
  async function insideLock(operation) {
    return locks.request(LOCK_NAME, { mode: "exclusive" }, operation);
  }
  return {
    get workspace() { return workspace; },
    async open() {
      workspace = await insideLock(async () => workspaceFactory(await store.load()));
      return workspace;
    },
    async reload() {
      workspace = workspaceFactory(await store.load());
      return workspace;
    },
    async mutate(method, ...args) {
      return insideLock(async () => {
        const candidate = workspaceFactory(await store.load());
        const result = candidate[method](...args);
        await store.save(candidate.snapshot());
        workspace = candidate;
        channel?.postMessage({ type: "workspace-updated", revision: result?.revision });
        return result;
      });
    },
    listen(onChange) {
      if (!channel) return () => {};
      const handler = () => onChange();
      channel.addEventListener("message", handler);
      return () => channel.removeEventListener("message", handler);
    },
  };
}
