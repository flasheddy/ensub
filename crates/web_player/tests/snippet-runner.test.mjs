import { describe, expect, test } from "bun:test";
import { createSnippetRunner } from "../js/snippet-runner.js";

class FakeAudio extends EventTarget {
  constructor() {
    super();
    this.src = "https://media.example.test/audio.mp3";
    this.currentSrc = this.src;
    this.currentTime = 0;
    this.duration = 10;
    this.paused = true;
    this.readyState = 1;
    this.error = null;
    this.pauseCalls = 0;
    this.listeners = new Map();
  }

  addEventListener(type, listener, options) {
    super.addEventListener(type, listener, options);
    const listeners = this.listeners.get(type) ?? new Set();
    listeners.add(listener);
    this.listeners.set(type, listeners);
  }

  removeEventListener(type, listener, options) {
    super.removeEventListener(type, listener, options);
    this.listeners.get(type)?.delete(listener);
  }

  listenerCount(type) {
    return this.listeners.get(type)?.size ?? 0;
  }

  load() {
    this.currentSrc = this.src;
  }

  play() {
    this.paused = false;
    return Promise.resolve();
  }

  pause() {
    this.pauseCalls += 1;
    this.paused = true;
    this.dispatchEvent(new Event("pause"));
  }
}

function frames() {
  let nextId = 1;
  const callbacks = new Map();
  const cancelled = [];
  return {
    request(callback) {
      const id = nextId++;
      callbacks.set(id, callback);
      return id;
    },
    cancel(id) {
      cancelled.push(id);
      callbacks.delete(id);
    },
    step() {
      const pending = [...callbacks.values()];
      callbacks.clear();
      pending.forEach((callback) => callback());
    },
    get active() { return callbacks.size; },
    cancelled,
  };
}

const descriptor = {
  audioSourceUrl: "https://media.example.test/audio.mp3",
  sliceStartMs: 500,
  sliceEndMs: 2_500,
};

async function start(runner, audio) {
  const result = runner.play(descriptor);
  audio.dispatchEvent(new Event("seeked"));
  await Promise.resolve();
  return { result };
}

describe("audio snippet runner", () => {
  test("stops within tolerance and tears down before its programmatic pause", async () => {
    const audio = new FakeAudio();
    const raf = frames();
    const runner = createSnippetRunner(audio, {
      requestFrame: (callback) => raf.request(callback),
      cancelFrame: (id) => raf.cancel(id),
    });
    const { result } = await start(runner, audio);

    audio.currentTime = 2.46;
    raf.step();

    await expect(result).resolves.toMatchObject({ status: "completed" });
    expect(audio.currentTime).toBe(2.5);
    expect(audio.pauseCalls).toBe(1);
    expect(raf.active).toBe(0);
    for (const event of ["loadedmetadata", "seeked", "timeupdate", "pause", "abort", "ended", "error"]) {
      expect(audio.listenerCount(event)).toBe(0);
    }
  });

  test("keeps playing at 51ms and stops at the inclusive 50ms boundary on timeupdate", async () => {
    const audio = new FakeAudio();
    const raf = frames();
    const runner = createSnippetRunner(audio, {
      requestFrame: (callback) => raf.request(callback),
      cancelFrame: (id) => raf.cancel(id),
    });
    const { result } = await start(runner, audio);

    audio.currentTime = 2.449;
    audio.dispatchEvent(new Event("timeupdate"));
    expect(runner.active).toBe(true);
    audio.currentTime = 2.45;
    audio.dispatchEvent(new Event("timeupdate"));

    await expect(result).resolves.toMatchObject({ status: "completed", reason: "boundary" });
    expect(audio.currentTime).toBe(2.5);
  });

  test("fails immediately when the active enclosure has already errored", async () => {
    const audio = new FakeAudio();
    const raf = frames();
    audio.error = { code: 4 };
    const runner = createSnippetRunner(audio, {
      requestFrame: (callback) => raf.request(callback),
      cancelFrame: (id) => raf.cancel(id),
    });

    const result = runner.play(descriptor);

    expect(runner.active).toBe(false);
    await expect(result).resolves.toEqual({ status: "audio_unavailable", reason: "existing_error" });
    expect(raf.active).toBe(0);
  });

  test("pause abort ended and error synchronously cancel RAF and every boundary listener", async () => {
    for (const [event, expectedStatus] of [
      ["pause", "interrupted"],
      ["abort", "audio_unavailable"],
      ["ended", "audio_unavailable"],
      ["error", "audio_unavailable"],
    ]) {
      const audio = new FakeAudio();
      const raf = frames();
      const runner = createSnippetRunner(audio, {
        requestFrame: (callback) => raf.request(callback),
        cancelFrame: (id) => raf.cancel(id),
      });
      const { result } = await start(runner, audio);

      audio.dispatchEvent(new Event(event));

      expect(raf.active).toBe(0);
      for (const name of ["loadedmetadata", "seeked", "timeupdate", "pause", "abort", "ended", "error"]) {
        expect(audio.listenerCount(name)).toBe(0);
      }
      await expect(result).resolves.toMatchObject({ status: expectedStatus, reason: event });
      audio.dispatchEvent(new Event("timeupdate"));
      expect(raf.active).toBe(0);
    }
  });

  test("a replacement replay invalidates and settles the previous generation once", async () => {
    const audio = new FakeAudio();
    const raf = frames();
    const runner = createSnippetRunner(audio, {
      requestFrame: (callback) => raf.request(callback),
      cancelFrame: (id) => raf.cancel(id),
    });
    const { result: first } = await start(runner, audio);
    const second = runner.play(descriptor);
    audio.dispatchEvent(new Event("seeked"));
    await Promise.resolve();

    await expect(first).resolves.toMatchObject({ status: "interrupted", reason: "replaced" });
    audio.dispatchEvent(new Event("error"));
    await expect(second).resolves.toMatchObject({ status: "audio_unavailable", reason: "error" });
    expect(raf.active).toBe(0);
  });
});
