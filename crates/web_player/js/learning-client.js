import { PlayerRuntimeError } from "./assets.js";
import { createStorageSynchronizer } from "./storage-sync.js";

export const LEARNING_STORAGE_KEY = "ensub.sandbox.v1";
export const LEARNING_LOCK_NAME = "ensub.sandbox.storage.v1";
export const V01_BACKUP_STORAGE_KEY = "ensub_v0.1_backup";

export function createLearningClient({ LearningClass, lexiconBytes, locks = globalThis.navigator?.locks }) {
  const coordinationWritable = typeof locks?.request === "function";
  const learning = new LearningClass(lexiconBytes, LEARNING_STORAGE_KEY, !coordinationWritable);
  const storageSynchronizer = createStorageSynchronizer({
    activeKey: LEARNING_STORAGE_KEY,
    backupKey: V01_BACKUP_STORAGE_KEY,
  });
  const runWrite = (operation) => locks.request(
    LEARNING_LOCK_NAME,
    { mode: "exclusive" },
    async () => {
      try {
        return await operation();
      } finally {
        await storageSynchronizer.publish();
      }
    },
  );
  let recovery = null;
  const isWritable = () => coordinationWritable && !(learning.isReadOnly?.() ?? false);
  const requireWritable = (operation) => {
    if (!coordinationWritable) {
      throw new PlayerRuntimeError(
        "writer_coordination_unavailable",
        `${operation} requires browser Web Locks support.`,
      );
    }
    if (learning.isReadOnly?.()) {
      throw new PlayerRuntimeError("storage_read_only", "Learning storage is read-only.");
    }
  };
  return {
    get writable() {
      return isWritable();
    },
    get recovery() {
      return recovery;
    },
    async initialize() {
      try {
        const status = coordinationWritable
          ? await runWrite(() => learning.initializeStorage())
          : learning.initializeStorage();
        return { status };
      } catch (error) {
        recovery = {
          status: "recovery_required",
          message: error?.message ?? "Browser storage migration failed.",
        };
        return recovery;
      }
    },
    rawSnapshot() {
      return learning.rawSnapshot();
    },
    lookupToken(surface) {
      return learning.lookupToken(surface);
    },
    dueCount(input) {
      return learning.dueCount(input);
    },
    dueReviews(input) {
      return learning.dueReviews(input);
    },
    revealReview(input) {
      return learning.revealReview(input);
    },
    prepareDisambiguation(input) {
      return learning.prepareDisambiguation(input);
    },
    validateDisambiguationResponse(input) {
      return learning.validateDisambiguationResponse(input);
    },
    capturePodcast(input) {
      requireWritable("Capture");
      return Promise.resolve(runWrite(() => learning.capturePodcast(input)));
    },
    review(input) {
      requireWritable("Review");
      return Promise.resolve(runWrite(() => learning.review(input)));
    },
    reset() {
      requireWritable("Reset");
      return Promise.resolve(runWrite(() => learning.reset()));
    },
  };
}
