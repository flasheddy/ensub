import { expect, test } from "bun:test";
import { createAudioHost } from "../js/audio-host.js";

class FakeAudio extends EventTarget {
  constructor() {
    super();
    this.src = "https://media.example.test/episode.mp3";
    this.currentSrc = this.src;
    this.currentTime = 12.25;
    this.duration = 120;
    this.playbackRate = 1.5;
    this.volume = 0.65;
    this.muted = true;
    this.paused = false;
    this.readyState = 1;
  }

  load() {
    this.currentSrc = this.src;
    queueMicrotask(() => this.dispatchEvent(new Event("loadedmetadata")));
  }

  play() {
    this.paused = false;
    this.dispatchEvent(new Event("play"));
    return Promise.resolve();
  }

  pause() {
    this.paused = true;
    this.dispatchEvent(new Event("pause"));
  }
}

test("timeupdate and seeked feed DOM audio timestamps directly to synchronization", () => {
  const audio = new FakeAudio();
  const syncs = [];
  createAudioHost(audio, {
    onEvent() {},
    onSync: (positionMs) => syncs.push(positionMs),
    requestFrame: () => 1,
    cancelFrame() {},
  });

  audio.currentTime = 5.125;
  audio.dispatchEvent(new Event("timeupdate"));
  audio.currentTime = 9.5;
  audio.dispatchEvent(new Event("seeked"));

  expect(syncs).toEqual([5_125, 9_500]);
});

test("snippet mode suppresses normal sync and restores the active episode checkpoint", async () => {
  const audio = new FakeAudio();
  const mediaEvents = [];
  const syncs = [];
  let frameId = 0;
  const frames = new Map();
  const host = createAudioHost(audio, {
    onEvent: (event) => mediaEvents.push(event),
    onSync: (position) => syncs.push(position),
    requestFrame(callback) { const id = ++frameId; frames.set(id, callback); return id; },
    cancelFrame(id) { frames.delete(id); },
  });

  const checkpoint = host.enterSnippetMode("episode-1");
  expect(checkpoint).toEqual({
    episode_id: "episode-1",
    currentTime_ms: 12_250,
    playbackRate: 1.5,
  });
  expect(host.sessionDepth).toBe(1);
  expect(audio.paused).toBe(true);
  expect(mediaEvents).toEqual([]);
  expect(syncs).toEqual([]);

  const snippet = host.playSnippet({
    audioSourceUrl: "https://media.example.test/review.mp3",
    sliceStartMs: 500,
    sliceEndMs: 2_500,
  });
  await Promise.resolve();
  audio.dispatchEvent(new Event("seeked"));
  await Promise.resolve();
  audio.dispatchEvent(new Event("error"));
  await expect(snippet).resolves.toMatchObject({ status: "audio_unavailable" });
  expect(mediaEvents).toEqual([]);
  expect(syncs).toEqual([]);

  const restored = await host.exitSnippetMode();

  expect(restored).toEqual(checkpoint);
  expect(host.sessionDepth).toBe(0);
  expect(audio.currentSrc).toBe("https://media.example.test/episode.mp3");
  expect(audio.currentTime).toBe(12.25);
  expect(audio.playbackRate).toBe(1.5);
  expect(audio.volume).toBe(0.65);
  expect(audio.muted).toBe(true);
  expect(audio.paused).toBe(true);
  expect(mediaEvents).not.toContain("play");
  expect(mediaEvents).toContain("pause");
  expect(syncs.at(-1)).toBe(12_250);
});

test("snippet sessions restore frames in LIFO order and lock ordinary transport", async () => {
  const audio = new FakeAudio();
  const host = createAudioHost(audio, {
    onEvent() {}, onSync() {}, requestFrame: () => 1, cancelFrame() {},
  });

  const episode = host.enterSnippetMode("episode-1");
  audio.currentTime = 30;
  audio.playbackRate = 0.75;
  const nested = host.enterSnippetMode("episode-2");
  host.seek(55);
  host.setRate(2);
  await host.toggle();

  expect(host.sessionDepth).toBe(2);
  expect(audio.currentTime).toBe(30);
  expect(audio.playbackRate).toBe(0.75);
  expect(audio.paused).toBe(true);
  await expect(host.exitSnippetMode()).resolves.toEqual(nested);
  expect(audio.currentTime).toBe(30);
  expect(audio.playbackRate).toBe(0.75);
  await expect(host.exitSnippetMode()).resolves.toEqual(episode);
  expect(audio.currentTime).toBe(12.25);
  expect(audio.playbackRate).toBe(1.5);
  expect(audio.paused).toBe(true);
});
