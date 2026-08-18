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
    dueCount: () => ({ dueCount: 0 }),
  };
  const settings = {
    load: () => ({ adapterId: "openai_chat_completions", endpointUrl: "https://provider.example.test/v1/chat/completions", model: "model" }),
    credential: () => "synthetic-secret",
    hasConsent: () => consent,
    grantConsent: () => { consent = true; },
    save: () => {},
  };
  const adapters = { openai_chat_completions: { send: async (input) => { sent.push(input); return sendImpl(input); } } };
  const episode = { episode: { identity: { internalId: "episode", feedUrl: "feed" } }, selectedTranscriptUrl: "transcript" };
  const workspace = {
    view: () => ({ revision: 1, feeds: [], episodes: [] }),
    preparePodcastCapture: () => ({ surface: "went", draft: { selectedSurface: "went", sentence: "We went home." } }),
  };
  const view = { elements: { audio: new FakeAudio() }, bind(value) { handlers = value; }, render(value) { state = value; }, updateSync() {} };
  const coordinator = { workspace, mutate: async (method) => method === "selectEpisode" ? { ...episode, transcriptState: "none" } : null };
  createController({ coordinator, learning, view, disambiguationSettings: settings, disambiguationAdapters: adapters });
  return { get handlers() { return handlers; }, get state() { return state; }, setState(value) { state = value; }, sent, prepared };
}

test("disambiguation waits for consent then sends only the prepared minimal request", async () => {
  const app = harness({ sendImpl: async () => JSON.stringify({ matchedSenseId: "sense-0-0", explanation: "Past tense of go.", confidence: "high" }) });
  await app.handlers.selectEpisode("episode");
  await app.handlers.lookupToken({ cueId: "cue", tokenIndex: 0 });
  await app.handlers.requestDisambiguation();
  expect(app.sent).toHaveLength(0);
  expect(app.state.lookup.ai.status).toBe("consent");
  expect(app.state.lookup.ai.prepared.request).toEqual(app.prepared.request);

  await app.handlers.confirmDisambiguation();
  expect(app.sent).toHaveLength(1);
  expect(app.sent[0].prepared.request).toEqual(app.prepared.request);
  expect(app.state.lookup.ai.status).toBe("ready");
  expect(app.state.lookup.result.status).toBe("found");
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

test("payload preparation failure is visible without changing local definitions", async () => {
  const app = harness({ prepareImpl: () => { throw new Error("Context unavailable."); } });
  await app.handlers.selectEpisode("episode");
  await app.handlers.lookupToken({ cueId: "cue", tokenIndex: 0 });
  await app.handlers.requestDisambiguation();
  expect(app.state.lookup.ai.status).toBe("failed");
  expect(app.state.lookup.ai.message).toContain("Context unavailable");
  expect(app.state.lookup.result.status).toBe("found");
});
