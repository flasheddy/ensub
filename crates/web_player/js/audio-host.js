const SYNC_EVENTS = new Set(["loadedmetadata", "durationchange", "timeupdate", "seeking", "seeked", "ratechange", "pause", "ended"]);
const MEDIA_EVENTS = [
  "loadstart", "loadedmetadata", "durationchange", "canplay", "play", "pause", "timeupdate",
  "seeking", "seeked", "ratechange", "volumechange", "waiting", "stalled", "ended", "emptied", "error",
];

export function createAudioHost(audio, { onEvent, onSync }) {
  let generation = 0;
  let frame = 0;

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
    frame = requestAnimationFrame(loop);
  }
  for (const event of MEDIA_EVENTS) {
    audio.addEventListener(event, () => {
      onEvent(event, values(), generation);
      if (event === "play") {
        cancelAnimationFrame(frame);
        frame = requestAnimationFrame(loop);
      }
      if (event === "pause" || event === "ended" || event === "emptied" || event === "error") {
        cancelAnimationFrame(frame);
      }
      if (SYNC_EVENTS.has(event)) sync();
    });
  }
  return {
    load(source, nextGeneration) {
      generation = nextGeneration;
      cancelAnimationFrame(frame);
      audio.src = source;
      audio.load();
    },
    toggle() { return audio.paused ? audio.play() : (audio.pause(), Promise.resolve()); },
    skip(seconds) { audio.currentTime = Math.max(0, Math.min(audio.duration || Infinity, audio.currentTime + seconds)); sync(); },
    seek(seconds) { audio.currentTime = Math.max(0, seconds); sync(); },
    setRate(rate) { audio.playbackRate = rate; },
    setVolume(volume) { audio.volume = volume; },
    toggleMute() { audio.muted = !audio.muted; },
    get generation() { return generation; },
  };
}
