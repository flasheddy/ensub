import assert from "node:assert/strict";
import test from "node:test";

import {
  analyzeVocabulary,
  ensureAnonymousSession,
  loadHistoryPage,
  toUserMessage,
} from "../js/supabase-api.js";

test("ensureAnonymousSession reuses an existing session", async () => {
  let signInCalls = 0;
  const existing = { user: { id: "existing-user" } };
  const client = {
    auth: {
      getSession: async () => ({ data: { session: existing }, error: null }),
      signInAnonymously: async () => {
        signInCalls += 1;
        return { data: { session: null }, error: null };
      },
    },
  };

  assert.equal(await ensureAnonymousSession(client), existing);
  assert.equal(signInCalls, 0);
});

test("ensureAnonymousSession creates an anonymous session when needed", async () => {
  const created = { user: { id: "anonymous-user" } };
  const client = {
    auth: {
      getSession: async () => ({ data: { session: null }, error: null }),
      signInAnonymously: async () => ({ data: { session: created }, error: null }),
    },
  };

  assert.equal(await ensureAnonymousSession(client), created);
});

test("loadHistoryPage requests a stable newest-first range", async () => {
  const calls = [];
  const rows = [{ id: "one", confidence: 0.8 }];
  const query = {
    select(columns) {
      calls.push(["select", columns]);
      return this;
    },
    order(column, options) {
      calls.push(["order", column, options]);
      return this;
    },
    range(from, to) {
      calls.push(["range", from, to]);
      return Promise.resolve({ data: rows, error: null });
    },
  };
  const client = { from: (table) => (calls.push(["from", table]), query) };

  const result = await loadHistoryPage(client, 20, 20);

  assert.equal(result.hasMore, false);
  assert.equal(result.records[0].id, "one");
  assert.deepEqual(calls, [
    ["from", "vocabulary_records"],
    [
      "select",
      "id,target_phrase,target_sentence,surrounding_context,lemma,part_of_speech,definition,nuance,confidence,created_at",
    ],
    ["order", "created_at", { ascending: false }],
    ["order", "id", { ascending: false }],
    ["range", 20, 40],
  ]);
});

test("analyzeVocabulary invokes the authenticated function and returns its record", async () => {
  const calls = [];
  const record = { id: "saved", targetPhrase: "set" };
  const client = {
    functions: {
      invoke: async (name, options) => {
        calls.push([name, options]);
        return { data: { record }, error: null };
      },
    },
  };
  const input = { targetPhrase: "set", targetSentence: "Set it down.", surroundingContext: null };

  assert.equal(await analyzeVocabulary(client, input, "request-id"), record);
  assert.deepEqual(calls, [["analyze-vocabulary", { body: { requestId: "request-id", ...input } }]]);
});

test("toUserMessage maps technical failures to actionable copy", () => {
  assert.equal(toUserMessage({ status: 429 }), "Analysis is busy right now. Wait a moment and try again.");
  assert.equal(toUserMessage({ status: 504 }), "The analysis took too long. Try again in a moment.");
  assert.equal(toUserMessage(new Error("secret provider detail")), "We could not analyze and save this capture. Please try again.");
});
