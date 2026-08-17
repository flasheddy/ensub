import { SandboxRuntimeError } from "./assets.js";

export function createWriterCoordinator({
  locks,
  lockName = "ensub.sandbox.storage.v1",
  timeoutMs = 10_000,
}) {
  const writable = typeof locks?.request === "function";

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
            operation,
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
