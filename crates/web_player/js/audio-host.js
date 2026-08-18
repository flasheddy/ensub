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
  let checkpoint = null;
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
      checkpoint = null;
      generation = nextGeneration;
      cancelFrame(frame);
      audio.src = source;
      audio.load();
    },
    toggle() { return audio.paused ? audio.play() : (audio.pause(), Promise.resolve()); },
    skip(seconds) { audio.currentTime = Math.max(0, Math.min(audio.duration || Infinity, audio.currentTime + seconds)); sync(); },
    seek(seconds) { audio.currentTime = Math.max(0, seconds); sync(); },
    setRate(rate) { audio.playbackRate = rate; },
    setVolume(volume) { audio.volume = volume; },
    toggleMute() { audio.muted = !audio.muted; },
    enterSnippetMode() {
      if (checkpoint) return { ...checkpoint };
      checkpoint = {
        source: audio.currentSrc || audio.src,
        currentTime: Number.isFinite(audio.currentTime) ? audio.currentTime : 0,
        paused: audio.paused,
        rate: audio.playbackRate,
        volume: audio.volume,
        muted: audio.muted,
        generation,
      };
      snippetMode = true;
      cancelFrame(frame);
      audio.pause();
      return { ...checkpoint };
    },
    playSnippet(descriptor) {
      if (!snippetMode) {
        return Promise.resolve({ status: "audio_unavailable", reason: "snippet_mode_inactive" });
      }
      return snippets.play(descriptor);
    },
    async exitSnippetMode() {
      if (!checkpoint) return;
      const restore = checkpoint;
      snippets.cancel("session_closed");
      audio.pause();
      await restoreSource(restore.source);
      audio.currentTime = restore.currentTime;
      audio.playbackRate = restore.rate;
      audio.volume = restore.volume;
      audio.muted = restore.muted;
      generation = restore.generation;
      checkpoint = null;
      snippetMode = false;
      if (!restore.paused) await audio.play();
      sync();
    },
    get generation() { return generation; },
    get snippetMode() { return snippetMode; },
  };
}
