import { createSnippetRunner } from "./snippet-runner.js";

const SYNC_EVENTS = new Set(["loadedmetadata", "durationchange", "timeupdate", "seeking", "seeked", "ratechange", "pause", "ended"]);
const MEDIA_EVENTS = [
  "loadstart", "loadedmetadata", "durationchange", "canplay", "play", "pause", "timeupdate",
  "seeking", "seeked", "ratechange", "volumechange", "waiting", "stalled", "ended", "emptied", "error",
];

export function createAudioHost(audio, {
  onEvent,
  onSync,
  requestFrame = globalThis.requestAnimationFrame.bind(globalThis),
  cancelFrame = globalThis.cancelAnimationFrame.bind(globalThis),
}) {
  let generation = 0;
  let frame = 0;
  let snippetMode = false;
  const sessionStack = [];
  const restoreStack = [];
  const snippets = createSnippetRunner(audio, { requestFrame, cancelFrame });

  function values() {
    return {
      currentTime: Number.isFinite(audio.currentTime) ? audio.currentTime : 0,
      duration: Number.isFinite(audio.duration) ? audio.duration : 0,
      rate: audio.playbackRate,
      volume: audio.volume,
      muted: audio.muted,
    };
  }
  function sync() {
    onSync(Math.max(0, Math.round(audio.currentTime * 1000)), generation);
  }
  function loop() {
    sync();
    frame = requestFrame(loop);
  }
  for (const event of MEDIA_EVENTS) {
    audio.addEventListener(event, () => {
      if (snippetMode) return;
      onEvent(event, values(), generation);
      if (event === "play") {
        cancelFrame(frame);
        frame = requestFrame(loop);
      }
      if (event === "pause" || event === "ended" || event === "emptied" || event === "error") {
        cancelFrame(frame);
      }
      if (SYNC_EVENTS.has(event)) sync();
    });
  }

  function restoreSource(source) {
    if ((audio.currentSrc || audio.src) === source) return Promise.resolve();
    return new Promise((resolve) => {
      const finish = () => {
        audio.removeEventListener("loadedmetadata", finish);
        audio.removeEventListener("error", finish);
        resolve();
      };
      audio.addEventListener("loadedmetadata", finish);
      audio.addEventListener("error", finish);
      audio.src = source;
      audio.load();
    });
  }

  return {
    load(source, nextGeneration) {
      snippets.cancel("episode_changed");
      snippetMode = false;
      sessionStack.length = 0;
      restoreStack.length = 0;
      generation = nextGeneration;
      cancelFrame(frame);
      audio.src = source;
      audio.load();
    },
    toggle() {
      if (snippetMode) return Promise.resolve();
      return audio.paused ? audio.play() : (audio.pause(), Promise.resolve());
    },
    skip(seconds) {
      if (snippetMode) return;
      audio.currentTime = Math.max(0, Math.min(audio.duration || Infinity, audio.currentTime + seconds));
      sync();
    },
    seek(seconds) {
      if (snippetMode) return;
      audio.currentTime = Math.max(0, seconds);
      sync();
    },
    setRate(rate) { if (!snippetMode) audio.playbackRate = rate; },
    setVolume(volume) { if (!snippetMode) audio.volume = volume; },
    toggleMute() { if (!snippetMode) audio.muted = !audio.muted; },
    enterSnippetMode(episodeId = null) {
      const session = {
        episode_id: episodeId,
        currentTime_ms: Math.max(0, Math.round((Number.isFinite(audio.currentTime) ? audio.currentTime : 0) * 1000)),
        playbackRate: audio.playbackRate,
      };
      sessionStack.push(session);
      restoreStack.push({
        source: audio.currentSrc || audio.src,
        volume: audio.volume,
        muted: audio.muted,
        generation,
      });
      snippetMode = true;
      cancelFrame(frame);
      audio.pause();
      return { ...session };
    },
    playSnippet(descriptor) {
      if (!snippetMode) {
        return Promise.resolve({ status: "audio_unavailable", reason: "snippet_mode_inactive" });
      }
      return snippets.play(descriptor);
    },
    cancelSnippet(reason = "cancelled") { snippets.cancel(reason); },
    async exitSnippetMode() {
      const session = sessionStack.pop();
      const restore = restoreStack.pop();
      if (!session || !restore) return null;
      snippets.cancel("session_closed");
      audio.pause();
      await restoreSource(restore.source);
      try { audio.currentTime = session.currentTime_ms / 1000; } catch { /* Failed media still restores as text-only state. */ }
      audio.playbackRate = session.playbackRate;
      audio.volume = restore.volume;
      audio.muted = restore.muted;
      generation = restore.generation;
      snippetMode = sessionStack.length > 0;
      if (!snippetMode) {
        onEvent("pause", values(), generation);
        sync();
      }
      return { ...session };
    },
    get generation() { return generation; },
    get snippetMode() { return snippetMode; },
    get sessionDepth() { return sessionStack.length; },
  };
}
