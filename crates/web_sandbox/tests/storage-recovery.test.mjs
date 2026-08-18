import { expect, test } from "bun:test";

import { createRawSnapshotBlob } from "../js/storage-recovery.js";

test("raw recovery export preserves the exact snapshot string", async () => {
  const exact = "{\n  \"format\": \"ensub-browser-storage\", \"schemaVersion\": 1\n}";
  const blob = createRawSnapshotBlob({ rawSnapshot: () => exact });

  expect(blob.type.startsWith("application/json")).toBe(true);
  expect(await blob.text()).toBe(exact);
});

test("raw recovery export rejects a missing snapshot", () => {
  expect(() => createRawSnapshotBlob({ rawSnapshot: () => null }))
    .toThrow("No browser learning snapshot is available");
});
