import { createAudioHost } from "./audio-host.js";
import { createOpenAiChatCompletionsAdapter } from "./disambiguation-adapter.js";
import { createDisambiguationSettings } from "./disambiguation-settings.js";
import { decodeUtf8, fetchBounded } from "./transport.js";
import { stepPlaybackRate } from "./shortcuts.js";
import { initialState, reduce } from "./state.js";

const FEED_LIMIT = 5 * 1024 * 1024;
const FIXTURE_LIMIT = 1024 * 1024;
const TRANSCRIPT_LIMIT = 10 * 1024 * 1024;

function errorState(error, offline = false) {
  if (offline) return "offline";
  if (error?.code === "browser_access_failed") return "browser_access_failed";
  if (error?.code?.startsWith("transcript_")) return "malformed";
  return "unavailable";
}

export function createController({
  coordinator,
  learning,
  view,
  fetchImpl = globalThis.fetch,
  clock = () => Date.now(),
  disambiguationSettings = createDisambiguationSettings(),
  disambiguationAdapters = { openai_chat_completions: createOpenAiChatCompletionsAdapter({ fetchImpl }) },
}) {
  let state = initialState();
  let feedGeneration = 0;
  let transcriptGeneration = 0;
  let mediaGeneration = 0;
  let feedAbort;
  let transcriptAbort;
  let reviewSession = 0;
  let disambiguationRequestId = 0;
  let disambiguationAbort;

  function dispatch(action) {
    state = reduce(state, action);
    view.render(state);
    return state;
  }
  const audio = createAudioHost(view.elements.audio, {
    onEvent(event, values, generation) { dispatch({ type: "media/event", event, values, generation }); },
    onSync(positionMs, generation) {
      if (generation !== mediaGeneration) return;
      try {
        const sync = coordinator.workspace.syncAt(positionMs);
        dispatch({ type: "sync/resolved", sync });
        view.updateSync(sync, state.follow);
      } catch { /* Episodes without cached transcripts still play. */ }
    },
  });

  async function loadFeed(url) {
    feedAbort?.abort();
    const request = new AbortController();
    feedAbort = request;
    const generation = ++feedGeneration;
    dispatch({ type: "feed/requested", generation });
    try {
      const bytes = await fetchBounded(url, { limit: FEED_LIMIT, signal: request.signal, fetchImpl });
      const workspace = await coordinator.mutate("importFeed", new URL(url, location.href).href, bytes, clock());
      dispatch({ type: "feed/succeeded", generation, workspace });
      const first = workspace.episodes[0];
      if (first) await selectEpisode(first.identity.internalId);
    } catch (error) {
      if (request.signal.aborted) return;
      dispatch({ type: "feed/failed", generation, status: errorState(error, !navigator.onLine), message: error.message });
    }
  }

  async function loadDemo() {
    feedAbort?.abort();
    const request = new AbortController();
    feedAbort = request;
    const generation = ++feedGeneration;
    const fixtureUrl = new URL("./assets/demo-fixture.json", location.href).href;
    dispatch({ type: "feed/requested", generation });
    try {
      const bytes = await fetchBounded(fixtureUrl, { limit: FIXTURE_LIMIT, signal: request.signal, fetchImpl });
      const opened = await coordinator.mutate("importDemoFixture", fixtureUrl, bytes, clock());
      dispatch({ type: "feed/succeeded", generation, workspace: coordinator.workspace.view() });
      await activateEpisode(opened);
    } catch (error) {
      if (request.signal.aborted) return;
      dispatch({ type: "feed/failed", generation, status: errorState(error, !navigator.onLine), message: error.message });
    }
  }

  async function activateEpisode(opened) {
    const episodeId = opened.episode.identity.internalId;
    const generation = ++transcriptGeneration;
    dispatch({ type: "transcript/requested", generation, episode: opened });
    mediaGeneration += 1;
    dispatch({ type: "media/source", generation: mediaGeneration, episodeId });
    audio.load(opened.episode.enclosureUrl, mediaGeneration);
    if (opened.transcript || opened.transcriptState !== "loading" || !opened.selectedTranscriptUrl) {
      dispatch({ type: "transcript/ready", generation, episode: opened });
      return;
    }
    await loadTranscript(opened, generation);
  }

  async function selectEpisode(episodeId) {
    if (state.review.phase !== "closed") await closeReview();
    disambiguationAbort?.abort();
    dispatch({ type: "lookup/closed" });
    transcriptAbort?.abort();
    const opened = await coordinator.mutate("selectEpisode", episodeId);
    dispatch({ type: "workspace/updated", workspace: coordinator.workspace.view() });
    await activateEpisode(opened);
  }

  async function loadTranscript(opened, generation) {
    const request = new AbortController();
    transcriptAbort = request;
    try {
      const bytes = await fetchBounded(opened.selectedTranscriptUrl, { limit: TRANSCRIPT_LIMIT, signal: request.signal, fetchImpl });
      const source = decodeUtf8(bytes);
      const ready = await coordinator.mutate(
        "cacheTranscript", opened.episode.identity.internalId, opened.selectedTranscriptUrl, source, clock(),
      );
      dispatch({ type: "workspace/updated", workspace: coordinator.workspace.view() });
      dispatch({ type: "transcript/ready", generation, episode: ready });
      try {
        const sync = coordinator.workspace.syncAt(Math.round(view.elements.audio.currentTime * 1000));
        dispatch({ type: "sync/resolved", sync });
        view.updateSync(sync, state.follow);
      } catch { /* The ready state already communicates empty transcripts. */ }
    } catch (error) {
      if (request.signal.aborted) return;
      dispatch({ type: "transcript/failed", generation, status: errorState(error, !navigator.onLine), message: error.message });
    }
  }

  async function selectTranscript(url) {
    dispatch({ type: "lookup/closed" });
    const episodeId = state.transcript.episode?.episode.identity.internalId;
    if (!episodeId) return;
    transcriptAbort?.abort();
    const generation = ++transcriptGeneration;
    const opened = await coordinator.mutate("selectTranscript", episodeId, url);
    dispatch({ type: "workspace/updated", workspace: coordinator.workspace.view() });
    dispatch({ type: "transcript/requested", generation, episode: opened });
    if (opened.transcript) dispatch({ type: "transcript/ready", generation, episode: opened });
    else await loadTranscript(opened, generation);
  }

  function mediaMilliseconds(seconds) {
    return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1000) : null;
  }

  async function lookupToken(selection) {
    disambiguationAbort?.abort();
    dispatch({ type: "lookup/requested", selection });
    try {
      const episode = state.transcript.episode;
      const prepared = coordinator.workspace.preparePodcastCapture({
        revision: state.workspace.revision,
        episodeId: episode.episode.identity.internalId,
        transcriptUrl: episode.selectedTranscriptUrl,
        cueId: selection.cueId,
        tokenIndex: selection.tokenIndex,
        playbackPositionMs: mediaMilliseconds(view.elements.audio.currentTime) ?? 0,
        durationMs: mediaMilliseconds(view.elements.audio.duration),
      });
      const result = learning.lookupToken(prepared.surface);
      dispatch({ type: "lookup/resolved", prepared, result });
    } catch (error) {
      dispatch({ type: "lookup/failed", message: error?.message ?? "Lookup failed." });
    }
  }

  async function captureLookup(selectedLemma) {
    if (!state.lookup.prepared) return;
    dispatch({ type: "lookup/capturing" });
    try {
      const result = await learning.capturePodcast({
        draft: state.lookup.prepared.draft,
        selectedLemma: selectedLemma || null,
        capturedAtMs: clock(),
      });
      dispatch({ type: "lookup/captured", result });
      await refreshDueCount();
    } catch (error) {
      dispatch({ type: "lookup/failed", message: error?.message ?? "Capture failed." });
    }
  }

  function providerConfig() {
    const config = disambiguationSettings.load();
    const credential = disambiguationSettings.credential();
    if (!config?.adapterId || !config.endpointUrl || !config.model || !credential) return null;
    return { config, credential };
  }

  async function sendDisambiguation(prepared, requestId) {
    const provider = providerConfig();
    if (!provider) {
      dispatch({ type: "lookup/aiSettings", message: "Configure the provider endpoint, model, and credential." });
      return;
    }
    const adapter = disambiguationAdapters[provider.config.adapterId];
    if (!adapter) {
      dispatch({ type: "lookup/aiSettings", message: "The configured provider adapter is unavailable." });
      return;
    }
    disambiguationAbort?.abort();
    const request = new AbortController();
    disambiguationAbort = request;
    dispatch({ type: "lookup/aiRequested", prepared, requestId });
    try {
      const responseJson = await adapter.send({
        endpointUrl: provider.config.endpointUrl,
        model: provider.config.model,
        credential: provider.credential,
        prepared,
        signal: request.signal,
      });
      const response = await Promise.resolve(learning.validateDisambiguationResponse({
        request: prepared.request,
        responseJson,
      }));
      dispatch({ type: "lookup/aiResolved", requestId, response });
    } catch (error) {
      if (request.signal.aborted) return;
      dispatch({ type: "lookup/aiFailed", requestId, message: error?.message ?? "The contextual explanation is unavailable." });
    }
  }

  async function requestDisambiguation() {
    if (!state.lookup.prepared || !["found", "ambiguous"].includes(state.lookup.result?.status)) return;
    const provider = providerConfig();
    if (!provider) {
      dispatch({ type: "lookup/aiSettings", message: "Configure the provider endpoint, model, and credential." });
      return;
    }
    try {
      const prepared = await Promise.resolve(learning.prepareDisambiguation({ draft: state.lookup.prepared.draft }));
      const requestId = ++disambiguationRequestId;
      if (!disambiguationSettings.hasConsent(provider.config.adapterId, provider.config.endpointUrl)) {
        dispatch({ type: "lookup/aiConsent", prepared, requestId });
        return;
      }
      await sendDisambiguation(prepared, requestId);
    } catch (error) {
      const requestId = ++disambiguationRequestId;
      dispatch({ type: "lookup/aiRequested", prepared: null, requestId });
      dispatch({ type: "lookup/aiFailed", requestId, message: error?.message ?? "The contextual request could not be prepared." });
    }
  }

  async function confirmDisambiguation() {
    const prepared = state.lookup.ai.prepared;
    const provider = providerConfig();
    if (!prepared || !provider) {
      dispatch({ type: "lookup/aiSettings", message: "Configure the provider before sending." });
      return;
    }
    disambiguationSettings.grantConsent(provider.config.adapterId, provider.config.endpointUrl);
    await sendDisambiguation(prepared, state.lookup.ai.requestId);
  }

  function saveDisambiguationSettings(input) {
    try {
      disambiguationSettings.save(input);
      dispatch({ type: "lookup/aiIdle" });
    } catch (error) {
      dispatch({ type: "lookup/aiSettings", message: error?.message ?? "Provider settings are invalid." });
    }
  }

  function showDisambiguationSettings() {
    dispatch({ type: "lookup/aiSettings" });
  }

  function cancelDisambiguation() {
    disambiguationAbort?.abort();
    dispatch({ type: "lookup/aiIdle" });
  }

  async function refreshDueCount() {
    try {
      const result = await Promise.resolve(learning.dueCount({ asOfMs: clock() }));
      dispatch({ type: "review/dueCount", dueCount: result.dueCount });
    } catch { /* Review availability must not affect playback or lookup. */ }
  }

  async function loadReviewQueue(sessionId, message = "") {
    const result = await Promise.resolve(learning.dueReviews({ asOfMs: state.review.asOfMs, limit: 20 }));
    dispatch({ type: "review/loaded", sessionId, cards: result.cards, message });
  }

  async function openReview() {
    if (state.review.phase !== "closed") return;
    const sessionId = ++reviewSession;
    audio.enterSnippetMode();
    dispatch({ type: "review/opened", sessionId, asOfMs: clock() });
    try {
      await loadReviewQueue(sessionId);
    } catch (error) {
      dispatch({ type: "review/failed", sessionId, message: error?.message ?? "Review cards are unavailable." });
    }
  }

  async function revealReview() {
    const sessionId = state.review.sessionId;
    const card = state.review.cards[state.review.index];
    if (!card || state.review.phase !== "prompt") return;
    dispatch({ type: "review/revealRequested", sessionId });
    try {
      const answer = await Promise.resolve(learning.revealReview({
        wordId: card.wordId,
        reviewToken: card.reviewToken,
      }));
      dispatch({ type: "review/revealed", sessionId, answer });
    } catch (error) {
      dispatch({ type: "review/revealFailed", sessionId, message: error?.message ?? "The answer is unavailable." });
    }
  }

  async function rateReview(rating) {
    const sessionId = state.review.sessionId;
    const card = state.review.cards[state.review.index];
    if (!card || state.review.answerStatus !== "ready" || state.review.saving) return;
    dispatch({ type: "review/ratingRequested", sessionId });
    try {
      const transition = await learning.review({
        wordId: card.wordId,
        reviewToken: card.reviewToken,
        rating,
        reviewedAtMs: clock(),
      });
      dispatch({ type: "review/rated", sessionId, transition });
      await refreshDueCount();
    } catch (error) {
      if (error?.code === "review_conflict") {
        try {
          await loadReviewQueue(sessionId, "This card changed in another session. The review queue was refreshed.");
        } catch (reloadError) {
          dispatch({ type: "review/failed", sessionId, message: reloadError?.message ?? "The refreshed queue is unavailable." });
        }
        return;
      }
      dispatch({ type: "review/ratingFailed", sessionId, message: error?.message ?? "The rating was not saved." });
    }
  }

  async function advanceReview() {
    const sessionId = state.review.sessionId;
    if (state.review.phase !== "rated") return;
    if (state.review.index + 1 < state.review.cards.length) {
      dispatch({ type: "review/advanced", sessionId });
      return;
    }
    try {
      await loadReviewQueue(sessionId);
    } catch (error) {
      dispatch({ type: "review/failed", sessionId, message: error?.message ?? "Review cards are unavailable." });
    }
  }

  function selectReviewContext(contextId) {
    dispatch({ type: "review/contextSelected", sessionId: state.review.sessionId, contextId });
  }

  async function playReviewSnippet() {
    const sessionId = state.review.sessionId;
    const card = state.review.cards[state.review.index];
    const context = card?.contexts.find((item) => item.contextId === state.review.selectedContextId);
    if (!context?.audioSlice) {
      dispatch({ type: "review/audio", sessionId, status: "audio_unavailable", message: "Audio is unavailable. Continue with the saved text." });
      return { status: "audio_unavailable", reason: "missing_slice" };
    }
    dispatch({ type: "review/audio", sessionId, status: "playing" });
    const result = await audio.playSnippet(context.audioSlice);
    if (result.status === "audio_unavailable") {
      dispatch({ type: "review/audio", sessionId, status: result.status, message: "Audio is unavailable. Continue with the saved text." });
    } else if (result.status === "completed") {
      dispatch({ type: "review/audio", sessionId, status: "ready" });
    }
    return result;
  }

  async function closeReview() {
    if (state.review.phase === "closed") return;
    const sessionId = state.review.sessionId;
    dispatch({ type: "review/exiting", sessionId });
    await audio.exitSnippetMode();
    dispatch({ type: "review/closed", sessionId });
    reviewSession += 1;
  }

  function seekAdjacentCue(direction) {
    const positionMs = Math.max(0, Math.round(view.elements.audio.currentTime * 1000));
    try {
      const target = direction > 0
        ? coordinator.workspace.nextCueAt(positionMs)
        : coordinator.workspace.previousCueAt(positionMs);
      if (target) audio.seek(target.startMs / 1000);
    } catch { /* Cue shortcuts are no-ops without a ready transcript or at a boundary. */ }
  }

  function handleShortcut(command) {
    switch (command.type) {
      case "toggle-playback":
        return audio.toggle().catch(() => {});
      case "next-cue":
        seekAdjacentCue(1);
        break;
      case "previous-cue":
        seekAdjacentCue(-1);
        break;
      case "skip":
        audio.skip(command.seconds);
        break;
      case "change-rate": {
        const rate = stepPlaybackRate(view.elements.audio.playbackRate, command.direction);
        audio.setRate(rate);
        dispatch({ type: "media/rate", rate: view.elements.audio.playbackRate });
        break;
      }
      case "toggle-review":
        return state.review.phase === "closed" ? openReview() : closeReview();
      default:
        break;
    }
    return undefined;
  }

  const handlers = {
    loadFeed,
    loadDemo,
    selectEpisode,
    selectTranscript,
    lookupToken,
    captureLookup,
    closeLookup() {
      disambiguationAbort?.abort();
      dispatch({ type: "lookup/closed" });
    },
    requestDisambiguation,
    confirmDisambiguation,
    saveDisambiguationSettings,
    showDisambiguationSettings,
    cancelDisambiguation,
    refreshDueCount,
    openReview,
    revealReview,
    rateReview,
    advanceReview,
    selectReviewContext,
    playReviewSnippet,
    closeReview,
    handleShortcut,
    togglePlayback: () => audio.toggle().catch(() => {}), skip: (seconds) => audio.skip(seconds),
    seekFraction: (fraction) => audio.seek(fraction * (view.elements.audio.duration || 0)),
    seekSeconds: (seconds) => audio.seek(seconds), setRate: (rate) => audio.setRate(rate),
    toggleMute: () => audio.toggleMute(), setVolume: (volume) => audio.setVolume(volume),
    manualFollow: () => dispatch({ type: "follow/manual" }),
    resumeFollow() {
      dispatch({ type: "follow/resume" });
      view.updateSync(state.sync, state.follow, true);
    },
  };
  view.bind(handlers);
  return {
    handlers,
    async start() {
      const workspace = coordinator.workspace.view();
      dispatch({ type: "feed/succeeded", generation: 0, workspace });
      dispatch({ type: "capabilities/updated", capabilities: { storage: "ready", online: navigator.onLine } });
      await refreshDueCount();
      const selected = workspace.selectedEpisodeId;
      if (selected) await selectEpisode(selected);
    },
    reloadWorkspace(workspace) { dispatch({ type: "workspace/updated", workspace }); },
    setOnline(online) { dispatch({ type: "capabilities/updated", capabilities: { online } }); },
    refreshDueCount,
  };
}
