import { describe, expect, test } from "bun:test";

import { createWriterCoordinator } from "../js/coordinator.js";

describe("createWriterCoordinator", () => {
  test("serializes each mutation under the named exclusive lock", async () => {
    const calls = [];
    const locks = {
      request: async (name, options, operation) => {
        calls.push({ name, options });
        return operation({ name });
      },
    };
    const coordinator = createWriterCoordinator({ locks });

    const value = await coordinator.run(() => "saved");

    expect(value).toBe("saved");
    expect(coordinator.writable).toBe(true);
    expect(calls).toEqual([{
      name: "ensub.sandbox.storage.v1",
      options: { mode: "exclusive", signal: expect.any(AbortSignal) },
    }]);
  });

  test("uses read-only mode when Web Locks are unavailable", async () => {
    const coordinator = createWriterCoordinator({ locks: undefined });

    expect(coordinator.writable).toBe(false);
    await expect(coordinator.run(() => "never"))
      .rejects.toMatchObject({ code: "writer_coordination_unavailable" });
  });

  test("times out a blocked writer request", async () => {
    const locks = { request: () => new Promise(() => {}) };
    const coordinator = createWriterCoordinator({ locks, timeoutMs: 5 });

    await expect(coordinator.run(() => "never"))
      .rejects.toMatchObject({ code: "writer_lock_timeout" });
  });
});
