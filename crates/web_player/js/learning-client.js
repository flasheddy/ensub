import { PlayerRuntimeError } from "./assets.js";

export const LEARNING_STORAGE_KEY = "ensub.sandbox.v1";
export const LEARNING_LOCK_NAME = "ensub.sandbox.storage.v1";

export function createLearningClient({ LearningClass, lexiconBytes, locks = globalThis.navigator?.locks }) {
  const writable = typeof locks?.request === "function";
  const learning = new LearningClass(lexiconBytes, LEARNING_STORAGE_KEY, !writable);
  return {
    writable,
    lookupToken(surface) {
      return learning.lookupToken(surface);
    },
    capturePodcast(input) {
      if (!writable) {
        throw new PlayerRuntimeError("writer_coordination_unavailable", "Capture requires browser Web Locks support.");
      }
      return locks.request(LEARNING_LOCK_NAME, { mode: "exclusive" }, () => learning.capturePodcast(input));
    },
  };
}
