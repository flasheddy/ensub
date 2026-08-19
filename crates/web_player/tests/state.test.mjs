import { describe, expect, test } from "bun:test";
import { initialState, reduce } from "../js/state.js";

describe("player reducer", () => {
  test("rejects stale feed and transcript completions", () => {
    let state = reduce(initialState(), { type: "feed/requested", generation: 3 });
    state = reduce(state, { type: "feed/succeeded", generation: 2, workspace: { episodes: [] } });
    expect(state.feed.status).toBe("loading");
    state = reduce(state, { type: "feed/succeeded", generation: 3, workspace: { episodes: [] } });
    expect(state.feed.status).toBe("ready");

    state = reduce(state, { type: "transcript/requested", generation: 8 });
    state = reduce(state, { type: "transcript/ready", generation: 7, episode: { transcript: { cues: [] } } });
    expect(state.transcript.status).toBe("loading");
  });

  test("manual navigation stops following until explicitly resumed", () => {
    let state = reduce(initialState(), { type: "sync/resolved", sync: { activeCueIndices: [4], anchorCueIndex: 4, precedingCueIndex: null } });
    expect(state.follow).toBe("following");
    state = reduce(state, { type: "follow/manual" });
    expect(state.follow).toBe("manual");
    state = reduce(state, { type: "sync/resolved", sync: { activeCueIndices: [5], anchorCueIndex: 5, precedingCueIndex: null } });
    expect(state.follow).toBe("manual");
    state = reduce(state, { type: "follow/resume" });
    expect(state.follow).toBe("following");
  });

  test("media generations ignore stale events and retain deliberate states", () => {
    let state = reduce(initialState(), { type: "media/source", generation: 2, episodeId: "episode-2" });
    state = reduce(state, { type: "media/event", generation: 1, event: "play" });
    expect(state.media.status).toBe("loading");
    state = reduce(state, { type: "media/event", generation: 2, event: "canplay" });
    state = reduce(state, { type: "media/event", generation: 2, event: "play" });
    expect(state.media.status).toBe("playing");
    state = reduce(state, { type: "media/event", generation: 2, event: "waiting" });
    expect(state.media.status).toBe("stalled");
  });

  test("review answers remain absent until reveal and rating advances atomically", () => {
    let state = reduce(initialState(), { type: "review/opened", sessionId: 4, asOfMs: 100 });
    state = reduce(state, {
      type: "review/loaded",
      sessionId: 4,
      cards: [{
        wordId: "word-1", reviewToken: "token", defaultContextId: "context-1",
        contexts: [{ contextId: "context-1", sentence: "We went home.", audioSlice: null }],
      }],
    });
    expect(state.review.phase).toBe("prompt");
    expect(state.review.answer).toBeNull();
    expect(state.review.selectedContextId).toBe("context-1");

    state = reduce(state, { type: "review/revealRequested", sessionId: 4 });
    expect(state.review.phase).toBe("revealing");
    expect(state.review.answerStatus).toBe("loading");
    state = reduce(state, {
      type: "review/revealed", sessionId: 4,
      answer: { lemma: "go", definition: "verb: move" },
    });
    expect(state.review.answerStatus).toBe("ready");
    expect(state.review.answer.lemma).toBe("go");

    state = reduce(state, { type: "review/ratingRequested", sessionId: 4 });
    expect(state.review.saving).toBe(true);
    state = reduce(state, {
      type: "review/rated", sessionId: 4,
      transition: { rating: 4, intervalDays: 1 },
    });
    expect(state.review.phase).toBe("rated");
    state = reduce(state, { type: "review/advanced", sessionId: 4 });
    expect(state.review.phase).toBe("complete");
  });

  test("review ignores stale session effects and preserves text review after audio failure", () => {
    let state = reduce(initialState(), { type: "review/opened", sessionId: 8, asOfMs: 100 });
    state = reduce(state, { type: "review/loaded", sessionId: 7, cards: [] });
    expect(state.review.phase).toBe("open");
    state = reduce(state, {
      type: "review/loaded", sessionId: 8,
      cards: [{ wordId: "word", reviewToken: "token", defaultContextId: "context", contexts: [{ contextId: "context", sentence: "Saved text." }] }],
    });
    state = reduce(state, { type: "review/audio", sessionId: 8, status: "audio_unavailable", message: "Audio unavailable." });

    expect(state.review.phase).toBe("prompt");
    expect(state.review.audio.status).toBe("audio_unavailable");
    expect(state.review.cards[0].contexts[0].sentence).toBe("Saved text.");
    state = reduce(state, { type: "review/exiting", sessionId: 8 });
    expect(state.review.phase).toBe("exit");
    state = reduce(state, { type: "review/closed", sessionId: 8 });
    expect(state.review.phase).toBe("closed");
  });

  test("review queue failure becomes an exit-capable terminal state", () => {
    let state = reduce(initialState(), { type: "review/opened", sessionId: 2, asOfMs: 100 });
    state = reduce(state, { type: "review/failed", sessionId: 2, message: "Queue unavailable." });
    expect(state.review.phase).toBe("complete");
    expect(state.review.message).toBe("Queue unavailable.");
  });
});
