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
    lookup: initialLookupState(),
    review: initialReviewState(),
  };
}

function initialLookupState() {
  return {
    status: "closed", selection: null, prepared: null, result: null, message: "",
    ai: { status: "idle", prepared: null, response: null, message: "", requestId: 0 },
  };
}

function initialReviewState(dueCount = 0) {
  return {
    phase: "closed", sessionId: 0, asOfMs: null, cards: [], index: 0,
    selectedContextId: null, answer: null, answerStatus: "idle", saving: false,
    transition: null, message: "", dueCount,
    audio: { status: "idle", message: "" },
  };
}

function currentReviewSession(state, action) {
  return action.sessionId === state.review.sessionId;
}

function promptReview(review, index) {
  const card = review.cards[index];
  if (!card) return { ...review, phase: "complete", index, answer: null, answerStatus: "idle", saving: false };
  return {
    ...review,
    phase: "prompt",
    index,
    selectedContextId: card.defaultContextId ?? card.contexts[0]?.contextId ?? null,
    answer: null,
    answerStatus: "idle",
    saving: false,
    transition: null,
    audio: { status: "idle", message: "" },
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
      return { ...state, lookup: { ...initialLookupState(), status: "pending", selection: action.selection } };
    case "lookup/resolved":
      return { ...state, lookup: { ...state.lookup, status: action.result.status, prepared: action.prepared, result: action.result, message: "" } };
    case "lookup/failed":
      return { ...state, lookup: { ...state.lookup, status: "failed", message: action.message } };
    case "lookup/capturing":
      return { ...state, lookup: { ...state.lookup, status: "capturing", message: "" } };
    case "lookup/captured":
      return { ...state, lookup: { ...state.lookup, status: action.result.status, message: "", capture: action.result } };
    case "lookup/closed":
      return { ...state, lookup: initialLookupState() };
    case "lookup/aiSettings":
      return { ...state, lookup: { ...state.lookup, ai: { ...state.lookup.ai, status: "settings", message: action.message ?? "" } } };
    case "lookup/aiConsent":
      return { ...state, lookup: { ...state.lookup, ai: { status: "consent", prepared: action.prepared, response: null, message: "", requestId: action.requestId } } };
    case "lookup/aiRequested":
      return { ...state, lookup: { ...state.lookup, ai: { ...state.lookup.ai, status: "requesting", prepared: action.prepared, response: null, message: "", requestId: action.requestId } } };
    case "lookup/aiResolved":
      if (action.requestId !== state.lookup.ai.requestId) return state;
      return { ...state, lookup: { ...state.lookup, ai: { ...state.lookup.ai, status: "ready", response: action.response, message: "" } } };
    case "lookup/aiFailed":
      if (action.requestId !== state.lookup.ai.requestId) return state;
      return { ...state, lookup: { ...state.lookup, ai: { ...state.lookup.ai, status: "failed", response: null, message: action.message } } };
    case "lookup/aiIdle":
      return { ...state, lookup: { ...state.lookup, ai: initialLookupState().ai } };
    case "review/dueCount":
      return { ...state, review: { ...state.review, dueCount: action.dueCount } };
    case "review/opened":
      return {
        ...state,
        review: {
          ...initialReviewState(state.review.dueCount),
          phase: "open", sessionId: action.sessionId, asOfMs: action.asOfMs,
        },
      };
    case "review/loaded": {
      if (!currentReviewSession(state, action)) return state;
      const review = { ...state.review, cards: action.cards, index: 0, message: action.message ?? "" };
      return { ...state, review: action.cards.length ? promptReview(review, 0) : { ...review, phase: "complete" } };
    }
    case "review/revealRequested":
      if (!currentReviewSession(state, action) || state.review.phase !== "prompt") return state;
      return { ...state, review: { ...state.review, phase: "revealing", answerStatus: "loading", message: "" } };
    case "review/revealed":
      if (!currentReviewSession(state, action) || state.review.phase !== "revealing") return state;
      return { ...state, review: { ...state.review, answer: action.answer, answerStatus: "ready", message: "" } };
    case "review/revealFailed":
      if (!currentReviewSession(state, action)) return state;
      return { ...state, review: { ...state.review, phase: "prompt", answer: null, answerStatus: "idle", message: action.message } };
    case "review/ratingRequested":
      if (!currentReviewSession(state, action) || state.review.answerStatus !== "ready" || state.review.saving) return state;
      return { ...state, review: { ...state.review, saving: true, message: "" } };
    case "review/rated":
      if (!currentReviewSession(state, action)) return state;
      return { ...state, review: { ...state.review, phase: "rated", saving: false, transition: action.transition } };
    case "review/ratingFailed":
      if (!currentReviewSession(state, action)) return state;
      return { ...state, review: { ...state.review, saving: false, message: action.message } };
    case "review/advanced":
      if (!currentReviewSession(state, action) || state.review.phase !== "rated") return state;
      return { ...state, review: promptReview(state.review, state.review.index + 1) };
    case "review/contextSelected":
      if (!currentReviewSession(state, action)) return state;
      return { ...state, review: { ...state.review, selectedContextId: action.contextId, audio: { status: "idle", message: "" } } };
    case "review/audio":
      if (!currentReviewSession(state, action)) return state;
      return { ...state, review: { ...state.review, audio: { status: action.status, message: action.message ?? "" } } };
    case "review/failed":
      if (!currentReviewSession(state, action)) return state;
      return { ...state, review: { ...state.review, phase: "complete", message: action.message } };
    case "review/exiting":
      if (!currentReviewSession(state, action)) return state;
      return { ...state, review: { ...state.review, phase: "exit" } };
    case "review/closed":
      if (!currentReviewSession(state, action)) return state;
      return { ...state, review: initialReviewState(state.review.dueCount) };
    default:
      return state;
  }
}
