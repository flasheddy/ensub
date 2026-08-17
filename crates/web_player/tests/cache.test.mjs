import { expect, test } from "bun:test";
import { createWorkspaceCoordinator } from "../js/cache.js";

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
