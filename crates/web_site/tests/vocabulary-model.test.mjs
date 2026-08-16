import assert from "node:assert/strict";
import test from "node:test";

import {
  formatConfidence,
  mapDatabaseRecord,
  mergeHistoryRecord,
  validateCaptureInput,
} from "../js/vocabulary-model.js";

const validInput = {
  targetPhrase: "  ran out  ",
  targetSentence: "  We ran out of time before the train arrived.  ",
  surroundingContext: "  The group was rushing to meet a deadline.  ",
};

test("validateCaptureInput trims valid fields", () => {
  assert.deepEqual(validateCaptureInput(validInput), {
    value: {
      targetPhrase: "ran out",
      targetSentence: "We ran out of time before the train arrived.",
      surroundingContext: "The group was rushing to meet a deadline.",
    },
    errors: {},
  });
});

test("validateCaptureInput rejects whitespace-only required fields", () => {
  const result = validateCaptureInput({
    targetPhrase: "  ",
    targetSentence: "\n",
    surroundingContext: "",
  });

  assert.deepEqual(result.errors, {
    targetPhrase: "Enter a word or phrase to analyze.",
    targetSentence: "Enter the sentence where it appears.",
  });
});

test("validateCaptureInput normalizes empty optional context to null", () => {
  const result = validateCaptureInput({
    targetPhrase: "set",
    targetSentence: "She set the book on the table.",
    surroundingContext: "   ",
  });

  assert.equal(result.value.surroundingContext, null);
});

test("validateCaptureInput enforces field limits", () => {
  const result = validateCaptureInput({
    targetPhrase: "p".repeat(121),
    targetSentence: "s".repeat(2001),
    surroundingContext: "c".repeat(5001),
  });

  assert.deepEqual(result.errors, {
    targetPhrase: "Keep the word or phrase within 120 characters.",
    targetSentence: "Keep the target sentence within 2,000 characters.",
    surroundingContext: "Keep the surrounding context within 5,000 characters.",
  });
});

test("mapDatabaseRecord converts database fields to the view model", () => {
  assert.deepEqual(
    mapDatabaseRecord({
      id: "5ae694d6-e085-4c0d-b3d8-beb7af2552e4",
      target_phrase: "ran out",
      target_sentence: "We ran out of time.",
      surrounding_context: null,
      lemma: "run out",
      part_of_speech: "phrasal verb",
      definition: "Use all of something.",
      nuance: "Signals complete depletion.",
      confidence: 0.94,
      created_at: "2026-08-16T03:00:00.000Z",
    }),
    {
      id: "5ae694d6-e085-4c0d-b3d8-beb7af2552e4",
      targetPhrase: "ran out",
      targetSentence: "We ran out of time.",
      surroundingContext: null,
      lemma: "run out",
      partOfSpeech: "phrasal verb",
      definition: "Use all of something.",
      nuance: "Signals complete depletion.",
      confidence: 0.94,
      createdAt: "2026-08-16T03:00:00.000Z",
    },
  );
});

test("formatConfidence renders a bounded percentage", () => {
  assert.equal(formatConfidence(0.944), "94%");
  assert.equal(formatConfidence(2), "100%");
  assert.equal(formatConfidence(-1), "0%");
});

test("mergeHistoryRecord prepends without duplicating an existing record", () => {
  const current = [{ id: "second" }, { id: "first" }];
  assert.deepEqual(mergeHistoryRecord(current, { id: "third" }), [
    { id: "third" },
    { id: "second" },
    { id: "first" },
  ]);
  assert.deepEqual(mergeHistoryRecord(current, { id: "second", updated: true }), [
    { id: "second", updated: true },
    { id: "first" },
  ]);
});
