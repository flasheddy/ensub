import { expect, test } from "bun:test";
import { createController } from "../js/controller.js";

class FakeAudio extends EventTarget {
  constructor() {
    super();
    this.src = "";
    this.currentSrc = "";
    this.currentTime = 0;
    this.duration = 120;
    this.playbackRate = 1;
    this.volume = 1;
    this.muted = false;
    this.paused = true;
  }
  load() { this.currentSrc = this.src; }
  play() { this.paused = false; this.dispatchEvent(new Event("play")); return Promise.resolve(); }
  pause() { this.paused = true; this.dispatchEvent(new Event("pause")); }
}

function demoOpen() {
  return {
    revision: 1,
    episode: {
      identity: { internalId: "demo-episode", feedUrl: "https://player.example/assets/demo-fixture.json" },
      title: "Demo Episode",
      enclosureUrl: "https://player.example/assets/demo.mp3",
      transcriptResources: [{ url: "https://player.example/assets/demo-fixture.json#transcript", format: "web_vtt" }],
    },
    selectedTranscriptUrl: "https://player.example/assets/demo-fixture.json#transcript",
    transcriptState: "ready",
    transcript: { cues: [] },
  };
}

function harness({ importError = null } = {}) {
  globalThis.navigator = { onLine: true };
  globalThis.location = new URL("https://player.example/app/");
  globalThis.requestAnimationFrame = () => 1;
  globalThis.cancelAnimationFrame = () => {};
  const audio = new FakeAudio();
  const calls = [];
  let handlers;
  let rendered;
  let workspaceView = { revision: 0, feeds: [], episodes: [] };
  const workspace = {
    view: () => workspaceView,
    syncAt: () => ({ activeCueIndices: [], anchorCueIndex: null, precedingCueIndex: null }),
  };
  const coordinator = {
    workspace,
    async mutate(method, ...args) {
      calls.push([method, ...args]);
      if (method !== "importDemoFixture") throw new Error(`unexpected mutation ${method}`);
      if (importError) throw importError;
      const opened = demoOpen();
      workspaceView = {
        revision: 1,
        feeds: [{ sourceUrl: opened.episode.identity.feedUrl, title: "Demo Feed" }],
        episodes: [opened.episode],
        selectedEpisodeId: opened.episode.identity.internalId,
      };
      return opened;
    },
  };
  const demo = { disabled: false };
  const fixtureBytes = new TextEncoder().encode('{"schema_version":1}');
  const fetches = [];
  const fetchImpl = async (url, options) => {
    fetches.push([url.href, options]);
    return new Response(fixtureBytes, {
      status: 200,
      headers: { "content-length": String(fixtureBytes.byteLength), "content-type": "application/json" },
    });
  };
  const view = {
    elements: { audio, demo },
    bind(value) { handlers = value; },
    render(value) {
      rendered = value;
      demo.disabled = value.workspace.feeds.length > 0 || value.feed.status === "loading";
    },
    updateSync() {},
  };
  const learning = { dueCount: () => ({ dueCount: 0 }) };
  createController({ coordinator, learning, view, fetchImpl, clock: () => 123_456 });
  return { audio, calls, demo, fetches, get handlers() { return handlers; }, get state() { return rendered; } };
}

test("Load Demo Episode passes bounded fixture bytes unchanged into Rust and opens the ready episode", async () => {
  const app = harness();
  await app.handlers.loadDemo();

  expect(app.fetches).toHaveLength(1);
  expect(app.fetches[0][0]).toBe("https://player.example/app/assets/demo-fixture.json");
  expect(app.calls).toHaveLength(1);
  expect(app.calls[0][0]).toBe("importDemoFixture");
  expect(app.calls[0][1]).toBe("https://player.example/app/assets/demo-fixture.json");
  expect(app.calls[0][2]).toBeInstanceOf(Uint8Array);
  expect(new TextDecoder().decode(app.calls[0][2])).toBe('{"schema_version":1}');
  expect(app.calls[0][3]).toBe(123_456);
  expect(app.state.workspace.feeds).toHaveLength(1);
  expect(app.state.transcript.status).toBe("ready");
  expect(app.audio.src).toBe("https://player.example/assets/demo.mp3");
});

test("a rejected demo fixture keeps the workspace empty and re-enables the Demo action", async () => {
  const app = harness({ importError: new Error("malformed demo fixture") });

  await app.handlers.loadDemo();

  expect(app.calls).toHaveLength(1);
  expect(app.state.workspace.feeds).toEqual([]);
  expect(app.state.workspace.episodes).toEqual([]);
  expect(app.state.feed.status).toBe("unavailable");
  expect(app.state.feed.message).toBe("malformed demo fixture");
  expect(app.demo.disabled).toBeFalse();
  expect(app.audio.src).toBe("");
});
