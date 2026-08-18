export function initialState() {
  return {
    capabilities: { online: globalThis.navigator?.onLine ?? true, installable: false, storage: "checking" },
    workspace: { revision: 0, feeds: [], episodes: [] },
    feed: { status: "empty", generation: 0, message: "" },
    transcript: { status: "none", generation: 0, episode: null, message: "" },
    media: {
      status: "idle", generation: 0, episodeId: null, currentTime: 0, duration: 0,
      rate: 1, volume: 1, muted: false,
    },
    sync: { activeCueIndices: [], anchorCueIndex: null, precedingCueIndex: null },
    follow: "following",
    lookup: { status: "closed", selection: null, prepared: null, result: null, message: "" },
  };
}

const MEDIA_STATUS = {
  loadstart: "loading", loadedmetadata: "ready", durationchange: "ready", canplay: "ready",
  play: "playing", pause: "paused", waiting: "stalled", stalled: "stalled", ended: "ended",
  emptied: "idle", error: "unavailable",
};

export function reduce(state, action) {
  switch (action.type) {
    case "capabilities/updated":
      return { ...state, capabilities: { ...state.capabilities, ...action.capabilities } };
    case "feed/requested":
      return { ...state, feed: { status: "loading", generation: action.generation, message: "" } };
    case "feed/succeeded":
      if (action.generation !== state.feed.generation) return state;
      return { ...state, workspace: action.workspace, feed: { status: "ready", generation: action.generation, message: "" } };
    case "feed/failed":
      if (action.generation !== state.feed.generation) return state;
      return { ...state, feed: { status: action.status, generation: action.generation, message: action.message } };
    case "workspace/updated":
      return { ...state, workspace: action.workspace };
    case "transcript/requested":
      return { ...state, transcript: { status: "loading", generation: action.generation, episode: action.episode ?? null, message: "" } };
    case "transcript/ready":
      if (action.generation !== state.transcript.generation) return state;
      return { ...state, transcript: { status: action.episode.transcriptState ?? "ready", generation: action.generation, episode: action.episode, message: "" } };
    case "transcript/failed":
      if (action.generation !== state.transcript.generation) return state;
      return { ...state, transcript: { ...state.transcript, status: action.status, message: action.message } };
    case "media/source":
      return { ...state, media: { ...initialState().media, status: "loading", generation: action.generation, episodeId: action.episodeId } };
    case "media/event": {
      if (action.generation !== state.media.generation) return state;
      const status = MEDIA_STATUS[action.event] ?? state.media.status;
      return { ...state, media: { ...state.media, status, ...action.values } };
    }
    case "sync/resolved":
      return { ...state, sync: action.sync };
    case "follow/manual":
      return state.follow === "manual" ? state : { ...state, follow: "manual" };
    case "follow/resume":
      return { ...state, follow: "following" };
    case "lookup/requested":
      return { ...state, lookup: { status: "pending", selection: action.selection, prepared: null, result: null, message: "" } };
    case "lookup/resolved":
      return { ...state, lookup: { ...state.lookup, status: action.result.status, prepared: action.prepared, result: action.result, message: "" } };
    case "lookup/failed":
      return { ...state, lookup: { ...state.lookup, status: "failed", message: action.message } };
    case "lookup/capturing":
      return { ...state, lookup: { ...state.lookup, status: "capturing", message: "" } };
    case "lookup/captured":
      return { ...state, lookup: { ...state.lookup, status: action.result.status, message: "", capture: action.result } };
    case "lookup/closed":
      return { ...state, lookup: initialState().lookup };
    default:
      return state;
  }
}
