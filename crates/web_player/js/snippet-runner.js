const BOUNDARY_TOLERANCE_MS = 50;
const EVENTS = ["loadedmetadata", "seeked", "timeupdate", "pause", "abort", "ended", "error"];

function validDescriptor(descriptor) {
  return descriptor
    && typeof descriptor.audioSourceUrl === "string"
    && descriptor.audioSourceUrl.length > 0
    && Number.isFinite(descriptor.sliceStartMs)
    && Number.isFinite(descriptor.sliceEndMs)
    && descriptor.sliceStartMs >= 0
    && descriptor.sliceEndMs > descriptor.sliceStartMs;
}

export function createSnippetRunner(audio, {
  requestFrame = globalThis.requestAnimationFrame.bind(globalThis),
  cancelFrame = globalThis.cancelAnimationFrame.bind(globalThis),
} = {}) {
  let generation = 0;
  let active = null;

  function removeListeners(operation) {
    for (const event of EVENTS) {
      audio.removeEventListener(event, operation.listeners[event]);
    }
  }

  function settle(operation, status, reason, { boundary = false } = {}) {
    if (active !== operation || operation.settled) return;
    operation.settled = true;
    active = null;
    generation += 1;
    if (operation.frame) cancelFrame(operation.frame);
    operation.frame = 0;
    removeListeners(operation);
    if (boundary) {
      audio.pause();
      audio.currentTime = operation.endSeconds;
    }
    operation.resolve({ status, reason });
  }

  function cancel(reason = "cancelled") {
    if (active) settle(active, "interrupted", reason);
  }

  function play(descriptor) {
    cancel("replaced");
    if (!validDescriptor(descriptor)) {
      return Promise.resolve({ status: "audio_unavailable", reason: "invalid_slice" });
    }
    const currentSource = audio.currentSrc || audio.src;
    if (currentSource === descriptor.audioSourceUrl && audio.error) {
      return Promise.resolve({ status: "audio_unavailable", reason: "existing_error" });
    }
    const operationGeneration = ++generation;
    let resolve;
    const result = new Promise((settled) => { resolve = settled; });
    const operation = {
      generation: operationGeneration,
      startSeconds: descriptor.sliceStartMs / 1000,
      endSeconds: descriptor.sliceEndMs / 1000,
      frame: 0,
      playStarted: false,
      settled: false,
      resolve,
      listeners: {},
    };
    active = operation;

    function current() {
      return active === operation && operation.generation === operationGeneration && !operation.settled;
    }
    function remainingMs() {
      return descriptor.sliceEndMs - audio.currentTime * 1000;
    }
    function checkBoundary() {
      if (!current()) return;
      if (remainingMs() <= BOUNDARY_TOLERANCE_MS) {
        settle(operation, "completed", "boundary", { boundary: true });
      }
    }
    function frameLoop() {
      if (!current()) return;
      checkBoundary();
      if (current()) operation.frame = requestFrame(frameLoop);
    }
    function beginPlayback() {
      if (!current() || operation.playStarted) return;
      operation.playStarted = true;
      Promise.resolve(audio.play()).then(() => {
        if (!current()) return;
        operation.frame = requestFrame(frameLoop);
      }).catch(() => {
        settle(operation, "audio_unavailable", "play_rejected");
      });
    }
    function seekToStart() {
      if (!current()) return;
      if (Math.abs(audio.currentTime - operation.startSeconds) <= 0.001) {
        beginPlayback();
        return;
      }
      try {
        audio.currentTime = operation.startSeconds;
      } catch {
        settle(operation, "audio_unavailable", "seek_failed");
      }
    }

    operation.listeners.loadedmetadata = seekToStart;
    operation.listeners.seeked = beginPlayback;
    operation.listeners.timeupdate = checkBoundary;
    operation.listeners.pause = () => settle(operation, "interrupted", "pause");
    operation.listeners.abort = () => settle(operation, "audio_unavailable", "abort");
    operation.listeners.ended = () => {
      if (remainingMs() <= BOUNDARY_TOLERANCE_MS) {
        settle(operation, "completed", "ended", { boundary: true });
      } else {
        settle(operation, "audio_unavailable", "ended");
      }
    };
    operation.listeners.error = () => settle(operation, "audio_unavailable", "error");
    for (const event of EVENTS) audio.addEventListener(event, operation.listeners[event]);

    if (currentSource !== descriptor.audioSourceUrl) {
      audio.src = descriptor.audioSourceUrl;
      audio.load();
    } else if (audio.readyState === 0) {
      audio.load();
    } else {
      seekToStart();
    }
    return result;
  }

  return {
    play,
    cancel,
    get active() { return Boolean(active); },
    get generation() { return generation; },
  };
}
