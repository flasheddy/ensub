import { expect, test } from "bun:test";
import { createController } from "../js/controller.js";

class FakeAudio extends EventTarget {
  constructor() {
    super();
    this.src = "episode.mp3";
    this.currentSrc = this.src;
    this.currentTime = 18;
    this.duration = 90;
    this.playbackRate = 1;
    this.volume = 1;
    this.muted = false;
    this.paused = true;
    this.readyState = 1;
  }
  load() { this.currentSrc = this.src; queueMicrotask(() => this.dispatchEvent(new Event("loadedmetadata"))); }
  play() { this.paused = false; this.dispatchEvent(new Event("play")); return Promise.resolve(); }
  pause() { this.paused = true; this.dispatchEvent(new Event("pause")); }
}

function harness({ reviewImpl, audioSlice = { audioSourceUrl: "episode.mp3", sliceStartMs: 1000, sliceEndMs: 2000 } } = {}) {
  globalThis.navigator = { onLine: true };
  globalThis.requestAnimationFrame = () => 1;
  globalThis.cancelAnimationFrame = () => {};
  const audio = new FakeAudio();
  let handlers;
  let state;
  const calls = [];
  let reviewed = false;
  const card = {
    wordId: "word-1", reviewToken: "rs1.current", defaultContextId: "context-1",
    contexts: [{
      contextId: "context-1", sentence: "We went home.", sourceLabel: "Episode",
      audioSlice,
    }],
  };
  const learning = {
    dueCount(input) { calls.push(["dueCount", input]); return { dueCount: 1 }; },
    dueReviews(input) { calls.push(["dueReviews", input]); return { asOfMs: input.asOfMs, cards: reviewed ? [] : [card] }; },
    revealReview(input) { calls.push(["revealReview", input]); return { ...input, term: "went", lemma: "go", phonetic: "", definition: "move" }; },
    review(input) {
      calls.push(["review", input]);
      const result = reviewImpl?.(input) ?? { ...input, intervalDays: 1 };
      reviewed = true;
      return result;
    },
  };
  const view = {
    elements: { audio },
    bind(value) { handlers = value; },
    render(value) { state = value; },
    updateSync() {},
  };
  const workspace = { view: () => ({ revision: 0, feeds: [], episodes: [] }) };
  createController({ coordinator: { workspace }, learning, view, clock: () => 123_456 });
  return { get handlers() { return handlers; }, get state() { return state; }, calls, card };
}

test("review uses a fixed session timestamp and rates only after reveal", async () => {
  const app = harness();
  await app.handlers.openReview();
  expect(app.state.review.phase).toBe("prompt");
  expect(app.calls[0]).toEqual(["dueReviews", { asOfMs: 123_456, limit: 20 }]);
  expect(app.state.review.answer).toBeNull();

  await app.handlers.revealReview();
  expect(app.state.review.answer.lemma).toBe("go");
  await app.handlers.rateReview(4);
  expect(app.calls.find(([name]) => name === "review")[1]).toEqual({
    wordId: "word-1", reviewToken: "rs1.current", rating: 4, reviewedAtMs: 123_456,
  });
  expect(app.state.review.phase).toBe("rated");
});

test("snippet replay never calls the scheduler and exit closes the session", async () => {
  const app = harness();
  await app.handlers.openReview();
  const first = app.handlers.playReviewSnippet();
  app.state;
  app.handlers.playReviewSnippet();
  expect(app.calls.some(([name]) => name === "review")).toBe(false);
  await app.handlers.closeReview();
  await first;
  expect(app.state.review.phase).toBe("closed");
});

test("the next action loads another batch at the original cutoff", async () => {
  const app = harness();
  await app.handlers.openReview();
  await app.handlers.revealReview();
  await app.handlers.rateReview(4);
  await app.handlers.advanceReview();
  expect(app.calls.filter(([name]) => name === "dueReviews")).toEqual([
    ["dueReviews", { asOfMs: 123_456, limit: 20 }],
    ["dueReviews", { asOfMs: 123_456, limit: 20 }],
  ]);
  expect(app.state.review.phase).toBe("complete");
});

test("missing audio keeps the saved sentence available for normal review", async () => {
  const app = harness({ audioSlice: null });
  await app.handlers.openReview();
  await expect(app.handlers.playReviewSnippet()).resolves.toMatchObject({ status: "audio_unavailable" });
  expect(app.state.review.audio.status).toBe("audio_unavailable");
  expect(app.state.review.cards[0].contexts[0].sentence).toBe("We went home.");
  await app.handlers.revealReview();
  await app.handlers.rateReview(3);
  expect(app.state.review.phase).toBe("rated");
});

test("a stale review token reloads the queue without retrying the rating", async () => {
  const conflict = Object.assign(new Error("Review state changed."), { code: "review_conflict" });
  const app = harness({ reviewImpl: () => { throw conflict; } });
  await app.handlers.openReview();
  await app.handlers.revealReview();
  await app.handlers.rateReview(3);

  expect(app.calls.filter(([name]) => name === "review")).toHaveLength(1);
  expect(app.calls.filter(([name]) => name === "dueReviews")).toHaveLength(2);
  expect(app.state.review.phase).toBe("prompt");
  expect(app.state.review.message).toContain("changed");
});
