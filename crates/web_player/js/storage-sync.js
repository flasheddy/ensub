const SYNC_ACK_TIMEOUT_MS = 250;

function applyStorageValue(storage, key, value) {
  if (value === null) storage.removeItem(key);
  else storage.setItem(key, value);
}

export function createStorageSynchronizer({
  activeKey,
  backupKey,
  channel = globalThis.BroadcastChannel
    ? new BroadcastChannel(`${activeKey}.storage-sync.v1`)
    : undefined,
  storage = globalThis.localStorage,
}) {
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
      applyStorageValue(storage, activeKey, message.active);
      applyStorageValue(storage, backupKey, message.backup);
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
        active: storage.getItem(activeKey),
        backup: storage.getItem(backupKey),
      });
      await Promise.race([
        acknowledged,
        new Promise((resolve) => setTimeout(resolve, SYNC_ACK_TIMEOUT_MS)),
      ]);
      acknowledgements.delete(messageId);
    },
  };
}
