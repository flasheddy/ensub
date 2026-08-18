import { SandboxRuntimeError } from "./assets.js";

const ACTIVE_STORAGE_KEY = "ensub.sandbox.v1";
const BACKUP_STORAGE_KEY = "ensub_v0.1_backup";
const SYNC_ACK_TIMEOUT_MS = 250;

function applyStorageValue(storage, key, value) {
  if (value === null) storage.removeItem(key);
  else storage.setItem(key, value);
}

function createStorageSynchronizer({ channel, storage }) {
  if (!channel || !storage) return { publish: async () => {} };

  const clientId = globalThis.crypto?.randomUUID?.()
    ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  const peers = new Set();
  const acknowledgements = new Map();
  let sequence = 0;

  channel.addEventListener("message", (event) => {
    const message = event.data;
    if (!message || message.sender === clientId) return;
    if (message.type === "hello") {
      peers.add(message.sender);
      channel.postMessage({ type: "hello_ack", sender: clientId, target: message.sender });
      return;
    }
    if (message.type === "hello_ack" && message.target === clientId) {
      peers.add(message.sender);
      return;
    }
    if (message.type === "snapshot") {
      peers.add(message.sender);
      applyStorageValue(storage, ACTIVE_STORAGE_KEY, message.active);
      applyStorageValue(storage, BACKUP_STORAGE_KEY, message.backup);
      channel.postMessage({
        type: "snapshot_ack",
        sender: clientId,
        target: message.sender,
        messageId: message.messageId,
      });
      return;
    }
    if (message.type === "snapshot_ack" && message.target === clientId) {
      acknowledgements.get(message.messageId)?.(message.sender);
    }
  });
  channel.postMessage({ type: "hello", sender: clientId });

  return {
    async publish() {
      const expected = new Set(peers);
      const messageId = `${clientId}:${sequence += 1}`;
      const acknowledged = new Promise((resolve) => {
        if (expected.size === 0) {
          resolve();
          return;
        }
        acknowledgements.set(messageId, (sender) => {
          expected.delete(sender);
          if (expected.size === 0) resolve();
        });
      });
      channel.postMessage({
        type: "snapshot",
        sender: clientId,
        messageId,
        active: storage.getItem(ACTIVE_STORAGE_KEY),
        backup: storage.getItem(BACKUP_STORAGE_KEY),
      });
      await Promise.race([
        acknowledged,
        new Promise((resolve) => setTimeout(resolve, SYNC_ACK_TIMEOUT_MS)),
      ]);
      acknowledgements.delete(messageId);
    },
  };
}

export function createWriterCoordinator({
  locks,
  channel = globalThis.BroadcastChannel
    ? new BroadcastChannel("ensub.sandbox.storage-sync.v1")
    : undefined,
  storage = globalThis.localStorage,
  lockName = "ensub.sandbox.storage.v1",
  timeoutMs = 10_000,
}) {
  const writable = typeof locks?.request === "function";
  const storageSynchronizer = createStorageSynchronizer({ channel, storage });

  return {
    writable,
    async run(operation) {
      if (!writable) {
        throw new SandboxRuntimeError(
          "writer_coordination_unavailable",
          "Writes require browser Web Locks support; this tab is read-only.",
        );
      }

      const controller = new AbortController();
      let timeout;
      const timedOut = new Promise((_, reject) => {
        timeout = setTimeout(() => {
          controller.abort();
          reject(new SandboxRuntimeError(
            "writer_lock_timeout",
            "Timed out waiting for another Ensub tab to finish writing.",
          ));
        }, timeoutMs);
      });
      try {
        return await Promise.race([
          locks.request(
            lockName,
            { mode: "exclusive", signal: controller.signal },
            async (lock) => {
              try {
                return await operation(lock);
              } finally {
                await storageSynchronizer.publish();
              }
            },
          ),
          timedOut,
        ]);
      } catch (error) {
        if (error?.code === "writer_lock_timeout") throw error;
        if (controller.signal.aborted || error?.name === "AbortError") {
          throw new SandboxRuntimeError(
            "writer_lock_timeout",
            "Timed out waiting for another Ensub tab to finish writing.",
            { cause: error },
          );
        }
        throw error;
      } finally {
        clearTimeout(timeout);
      }
    },
  };
}
