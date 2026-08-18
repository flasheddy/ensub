import { expect, test } from "bun:test";
import { createLearningClient, LEARNING_LOCK_NAME } from "../js/learning-client.js";

test("review reads are unlocked while rating uses the shared exclusive lock", async () => {
  const calls = [];
  class LearningClass {
    dueCount(input) { calls.push(["dueCount", input]); return { dueCount: 2 }; }
    dueReviews(input) { calls.push(["dueReviews", input]); return { cards: [] }; }
    revealReview(input) { calls.push(["revealReview", input]); return { lemma: "go" }; }
    prepareDisambiguation(input) { calls.push(["prepareDisambiguation", input]); return { request: input }; }
    validateDisambiguationResponse(input) { calls.push(["validateDisambiguationResponse", input]); return { explanation: "clear" }; }
    review(input) { calls.push(["review", input]); return { intervalDays: 1 }; }
    reset() { calls.push(["reset"]); }
  }
  const locks = {
    request(name, options, callback) {
      calls.push(["lock", name, options]);
      return callback();
    },
  };
  const client = createLearningClient({ LearningClass, lexiconBytes: new Uint8Array(), locks });

  expect(client.dueCount({ asOfMs: 1 })).toEqual({ dueCount: 2 });
  expect(client.dueReviews({ asOfMs: 1, limit: 50 })).toEqual({ cards: [] });
  expect(client.revealReview({ wordId: "word", reviewToken: "token" })).toEqual({ lemma: "go" });
  expect(client.prepareDisambiguation({ draft: "draft" })).toEqual({ request: { draft: "draft" } });
  expect(client.validateDisambiguationResponse({ request: {}, responseJson: "{}" })).toEqual({ explanation: "clear" });
  await expect(client.review({ wordId: "word", reviewToken: "token", rating: 4, reviewedAtMs: 1 }))
    .resolves.toEqual({ intervalDays: 1 });
  await client.reset();

  expect(calls.filter(([name]) => name === "lock")).toEqual([
    ["lock", LEARNING_LOCK_NAME, { mode: "exclusive" }],
    ["lock", LEARNING_LOCK_NAME, { mode: "exclusive" }],
  ]);
  expect(calls).toContainEqual(["reset"]);
});

test("storage initialization uses the shared lock and exposes recovery export", async () => {
  const calls = [];
  class HealthyLearning {
    initializeStorage() { calls.push(["initializeStorage"]); return "migrated"; }
    isReadOnly() { return false; }
    rawSnapshot() { return null; }
  }
  const locks = {
    request(name, options, callback) {
      calls.push(["lock", name, options]);
      return callback();
    },
  };
  const healthy = createLearningClient({
    LearningClass: HealthyLearning,
    lexiconBytes: new Uint8Array(),
    locks,
  });

  await expect(healthy.initialize()).resolves.toEqual({ status: "migrated" });
  expect(healthy.writable).toBe(true);
  expect(calls).toEqual([
    ["lock", LEARNING_LOCK_NAME, { mode: "exclusive" }],
    ["initializeStorage"],
  ]);

  const exact = "{\n  \"schemaVersion\": 1\n}";
  class RecoveryLearning {
    initializeStorage() { throw new Error("migration failed"); }
    isReadOnly() { return true; }
    rawSnapshot() { return exact; }
  }
  const recovery = createLearningClient({
    LearningClass: RecoveryLearning,
    lexiconBytes: new Uint8Array(),
    locks,
  });

  await expect(recovery.initialize()).resolves.toMatchObject({
    status: "recovery_required",
    message: "migration failed",
  });
  expect(recovery.writable).toBe(false);
  expect(recovery.rawSnapshot()).toBe(exact);
  expect(() => recovery.capturePodcast({})).toThrow("Learning storage is read-only");
});
