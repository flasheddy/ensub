import { expect, test } from "bun:test";

import { SandboxClient } from "../js/sandbox-client.js";

test("initialization uses the coordinator and latches recovery state", async () => {
  const calls = [];
  const coordinator = {
    writable: true,
    run(operation) {
      calls.push("lock");
      return Promise.resolve(operation());
    },
  };
  class HealthySandbox {
    initializeStorage() { calls.push("initialize"); return "current"; }
    isReadOnly() { return false; }
    rawSnapshot() { return null; }
  }
  const healthy = SandboxClient.open({
    SandboxClass: HealthySandbox,
    lexiconBytes: new Uint8Array(),
    storageKey: "ensub.test",
    coordinator,
  });

  await expect(healthy.initialize()).resolves.toEqual({ status: "current" });
  expect(healthy.writable).toBe(true);
  expect(calls).toEqual(["lock", "initialize"]);

  const exact = "{\n  \"schemaVersion\": 1\n}";
  class RecoverySandbox {
    initializeStorage() { throw new Error("migration failed"); }
    isReadOnly() { return true; }
    rawSnapshot() { return exact; }
  }
  const recovery = SandboxClient.open({
    SandboxClass: RecoverySandbox,
    lexiconBytes: new Uint8Array(),
    storageKey: "ensub.test",
    coordinator,
  });

  await expect(recovery.initialize()).resolves.toMatchObject({
    status: "recovery_required",
    message: "migration failed",
  });
  expect(recovery.writable).toBe(false);
  expect(recovery.rawSnapshot()).toBe(exact);
});
