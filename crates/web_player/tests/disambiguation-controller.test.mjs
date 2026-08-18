import { expect, test } from "bun:test";
import { createController } from "../js/controller.js";

class FakeAudio extends EventTarget {
  currentTime = 4;
  duration = 60;
  playbackRate = 1;
  volume = 1;
  muted = false;
  paused = true;
  src = "episode.mp3";
  currentSrc = this.src;
  load() {}
  play() { return Promise.resolve(); }
  pause() {}
}

function harness({ sendImpl = async () => "{}", prepareImpl } = {}) {
  globalThis.navigator = { onLine: true };
  globalThis.requestAnimationFrame = () => 1;
  globalThis.cancelAnimationFrame = () => {};
  let handlers;
  let state;
  const sent = [];
  const planned = [];
  const captured = [];
  let consent = false;
  const prepared = {
    request: {
      selectedWord: "went", savedSentence: "We went home.",
      candidateSenses: [{ senseId: "sense-0-0", lemma: "go", partOfSpeech: "verb", definition: "move" }],
      episodeLabel: "Synthetic Signal - Episode 1",
    },
    systemPrompt: "schema prompt",
    userPrompt: "minimal lexical JSON",
  };
  const learning = {
    lookupToken: () => ({ status: "found", entry: { lemma: "go", phonetic: "", definitions: [{ partOfSpeech: "verb", text: "move" }] } }),
    prepareDisambiguation: () => prepareImpl?.() ?? prepared,
    validateDisambiguationResponse: ({ responseJson }) => ({ ...JSON.parse(responseJson) }),
    capturePodcast: async (input) => {
      captured.push(input);
      return { status: "created_card", wordId: "word-go", contextId: "context-1" };
    },
    dueCount: () => ({ dueCount: 0 }),
    dueReviews: ({ asOfMs }) => ({ asOfMs, cards: [] }),
  };
  const settings = {
    load: () => ({ adapterId: "openai_chat_completions", endpointUrl: "https://provider.example.test/v1/chat/completions", model: "model" }),
    credential: () => "synthetic-secret",
    hasConsent: () => consent,
    grantConsent: () => { consent = true; },
    save: () => {},
  };
  const adapters = {
    openai_chat_completions: {
      planRequest(input) {
        const request = {
          endpointUrl: input.endpointUrl,
          method: "POST",
          contentType: "application/json",
          bodyText: JSON.stringify({ model: input.model, lexicalData: input.prepared.request }, null, 2),
        };
        planned.push(request);
        return request;
      },
      async send(input) { sent.push(input); return sendImpl(input); },
    },
  };
  const episode = { episode: { identity: { internalId: "episode", feedUrl: "feed" } }, selectedTranscriptUrl: "transcript" };
  const workspace = {
    view: () => ({ revision: 1, feeds: [], episodes: [] }),
    preparePodcastCapture: () => ({ surface: "went", draft: { selectedSurface: "went", sentence: "We went home." } }),
  };
  const view = { elements: { audio: new FakeAudio() }, bind(value) { handlers = value; }, render(value) { state = value; }, updateSync() {} };
  const coordinator = { workspace, mutate: async (method) => method === "selectEpisode" ? { ...episode, transcriptState: "none" } : null };
  createController({ coordinator, learning, view, disambiguationSettings: settings, disambiguationAdapters: adapters });
  return { get handlers() { return handlers; }, get state() { return state; }, setState(value) { state = value; }, sent, planned, captured, prepared };
}

test("capture shortcut commits the prepared lookup through the atomic capture path", async () => {
  const app = harness();
  await app.handlers.selectEpisode("episode");
  await app.handlers.lookupToken({ cueId: "cue", tokenIndex: 0 });

  await app.handlers.handleShortcut({ type: "capture-lookup", selectedLemma: "go" });

  expect(app.captured).toHaveLength(1);
  expect(app.captured[0]).toMatchObject({
    draft: { selectedSurface: "went", sentence: "We went home." },
    selectedLemma: "go",
  });
  expect(app.state.lookup.status).toBe("created_card");
});

test("disambiguation waits for consent then sends only the prepared minimal request", async () => {
  const app = harness({ sendImpl: async () => JSON.stringify({ matchedSenseId: "sense-0-0", explanation: "Past tense of go.", confidence: "high" }) });
  await app.handlers.selectEpisode("episode");
  await app.handlers.lookupToken({ cueId: "cue", tokenIndex: 0 });
  await app.handlers.requestDisambiguation();
  expect(app.sent).toHaveLength(0);
  expect(app.state.lookup.ai.status).toBe("consent");
  expect(app.state.lookup.ai.prepared.request).toEqual(app.prepared.request);
  expect(app.state.lookup.ai.transportRequest).toEqual(app.planned[0]);
  expect(app.state.lookup.ai.transportRequest.bodyText).not.toContain("synthetic-secret");

  await app.handlers.confirmDisambiguation();
  expect(app.sent).toHaveLength(1);
  expect(app.sent[0].request).toEqual(app.planned[0]);
  expect(app.state.lookup.ai.status).toBe("ready");
  expect(app.state.lookup.result.status).toBe("found");
});

test("consent is reused only after another explicit request", async () => {
  const app = harness({ sendImpl: async () => JSON.stringify({ matchedSenseId: null, explanation: "Ambiguous.", confidence: "low" }) });
  await app.handlers.selectEpisode("episode");
  await app.handlers.lookupToken({ cueId: "cue", tokenIndex: 0 });
  await app.handlers.requestDisambiguation();
  await app.handlers.confirmDisambiguation();

  await app.handlers.requestDisambiguation();

  expect(app.sent).toHaveLength(2);
  expect(app.planned).toHaveLength(2);
  expect(app.state.lookup.ai.status).toBe("ready");
});

test("provider failure leaves the local lookup result intact", async () => {
  const app = harness({ sendImpl: async () => { throw Object.assign(new Error("Provider unavailable."), { code: "provider_http_error" }); } });
  await app.handlers.selectEpisode("episode");
  await app.handlers.lookupToken({ cueId: "cue", tokenIndex: 0 });
  await app.handlers.requestDisambiguation();
  await app.handlers.confirmDisambiguation();
  expect(app.state.lookup.ai.status).toBe("failed");
  expect(app.state.lookup.result.status).toBe("found");
  expect(app.state.lookup.prepared.surface).toBe("went");
});

test("opening review aborts an in-flight contextual request and closes lookup", async () => {
  let requestSignal;
  const app = harness({
    sendImpl: ({ signal }) => new Promise((resolve) => {
      requestSignal = signal;
      signal.addEventListener("abort", () => resolve("{}"), { once: true });
      setTimeout(() => resolve("{}"), 100);
    }),
  });
  await app.handlers.selectEpisode("episode");
  await app.handlers.lookupToken({ cueId: "cue", tokenIndex: 0 });
  await app.handlers.requestDisambiguation();
  const sending = app.handlers.confirmDisambiguation();
  await Promise.resolve();

  await app.handlers.openReview();

  expect(requestSignal.aborted).toBe(true);
  expect(app.state.lookup.status).toBe("closed");
  await sending;
});

test("payload preparation failure is visible without changing local definitions", async () => {
  const app = harness({ prepareImpl: () => { throw new Error("Context unavailable."); } });
  await app.handlers.selectEpisode("episode");
  await app.handlers.lookupToken({ cueId: "cue", tokenIndex: 0 });
  await app.handlers.requestDisambiguation();
  expect(app.state.lookup.ai.status).toBe("failed");
  expect(app.state.lookup.ai.message).toContain("Context unavailable");
  expect(app.state.lookup.result.status).toBe("found");
});
