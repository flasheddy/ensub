export const CAPTURE_LIMITS = Object.freeze({
  targetPhrase: 120,
  targetSentence: 2_000,
  surroundingContext: 5_000,
});

export function validateCaptureInput(input) {
  const value = {
    targetPhrase: String(input.targetPhrase ?? "").trim(),
    targetSentence: String(input.targetSentence ?? "").trim(),
    surroundingContext: String(input.surroundingContext ?? "").trim() || null,
  };
  const errors = {};

  if (!value.targetPhrase) {
    errors.targetPhrase = "Enter a word or phrase to analyze.";
  } else if (value.targetPhrase.length > CAPTURE_LIMITS.targetPhrase) {
    errors.targetPhrase = "Keep the word or phrase within 120 characters.";
  }

  if (!value.targetSentence) {
    errors.targetSentence = "Enter the sentence where it appears.";
  } else if (value.targetSentence.length > CAPTURE_LIMITS.targetSentence) {
    errors.targetSentence = "Keep the target sentence within 2,000 characters.";
  }

  if (
    value.surroundingContext &&
    value.surroundingContext.length > CAPTURE_LIMITS.surroundingContext
  ) {
    errors.surroundingContext = "Keep the surrounding context within 5,000 characters.";
  }

  return { value, errors };
}

export function mapDatabaseRecord(record) {
  return {
    id: record.id,
    targetPhrase: record.target_phrase,
    targetSentence: record.target_sentence,
    surroundingContext: record.surrounding_context,
    lemma: record.lemma,
    partOfSpeech: record.part_of_speech,
    definition: record.definition,
    nuance: record.nuance,
    confidence: Number(record.confidence),
    createdAt: record.created_at,
  };
}

export function formatConfidence(confidence) {
  const bounded = Math.min(1, Math.max(0, Number(confidence)));
  return `${Math.round(bounded * 100)}%`;
}

export function mergeHistoryRecord(records, record) {
  return [record, ...records.filter((item) => item.id !== record.id)];
}
