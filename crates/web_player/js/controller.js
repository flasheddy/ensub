import { createAudioHost } from "./audio-host.js";
import { decodeUtf8, fetchBounded } from "./transport.js";
import { initialState, reduce } from "./state.js";

const FEED_LIMIT = 5 * 1024 * 1024;
const TRANSCRIPT_LIMIT = 10 * 1024 * 1024;

function errorState(error, offline = false) {
  if (offline) return "offline";
  if (error?.code === "browser_access_failed") return "browser_access_failed";
  if (error?.code?.startsWith("transcript_")) return "malformed";
  return "unavailable";
}

export function createController({ coordinator, view, fetchImpl = globalThis.fetch, clock = () => Date.now() }) {
  let state = initialState();
  let feedGeneration = 0;
  let transcriptGeneration = 0;
  let mediaGeneration = 0;
  let feedAbort;
  let transcriptAbort;

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

  async function selectEpisode(episodeId) {
    transcriptAbort?.abort();
    const generation = ++transcriptGeneration;
    const opened = await coordinator.mutate("selectEpisode", episodeId);
    dispatch({ type: "workspace/updated", workspace: coordinator.workspace.view() });
    dispatch({ type: "transcript/requested", generation, episode: opened });
    mediaGeneration += 1;
    dispatch({ type: "media/source", generation: mediaGeneration, episodeId });
    audio.load(opened.episode.enclosureUrl, mediaGeneration);
    if (opened.transcript) {
      dispatch({ type: "transcript/ready", generation, episode: opened });
      return;
    }
    if (opened.transcriptState !== "loading" || !opened.selectedTranscriptUrl) {
      dispatch({ type: "transcript/ready", generation, episode: opened });
      return;
    }
    await loadTranscript(opened, generation);
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

  const handlers = {
    loadFeed,
    loadDemo: () => loadFeed(new URL("./assets/demo/feed.xml", location.href).href),
    selectEpisode,
    selectTranscript,
    togglePlayback: () => audio.toggle().catch(() => {}), skip: (seconds) => audio.skip(seconds),
    seekFraction: (fraction) => audio.seek(fraction * (view.elements.audio.duration || 0)),
    seekSeconds: (seconds) => audio.seek(seconds), setRate: (rate) => audio.setRate(rate),
    toggleMute: () => audio.toggleMute(), setVolume: (volume) => audio.setVolume(volume),
    manualFollow: () => dispatch({ type: "follow/manual" }),
    resumeFollow() {
      dispatch({ type: "follow/resume" });
      view.updateSync(state.sync, state.follow);
    },
  };
  view.bind(handlers);
  return {
    handlers,
    async start() {
      const workspace = coordinator.workspace.view();
      dispatch({ type: "feed/succeeded", generation: 0, workspace });
      dispatch({ type: "capabilities/updated", capabilities: { storage: "ready", online: navigator.onLine } });
      const selected = workspace.selectedEpisodeId;
      if (selected) await selectEpisode(selected);
    },
    reloadWorkspace(workspace) { dispatch({ type: "workspace/updated", workspace }); },
    setOnline(online) { dispatch({ type: "capabilities/updated", capabilities: { online } }); },
  };
}
