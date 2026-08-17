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
});
