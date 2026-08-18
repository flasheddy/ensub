export const PLAYBACK_RATES = Object.freeze([0.75, 1, 1.25, 1.5, 1.75, 2]);

const SHORTCUTS = new Map([
  [" ", { type: "toggle-playback" }],
  ["j", { type: "next-cue" }],
  ["ArrowDown", { type: "next-cue" }],
  ["k", { type: "previous-cue" }],
  ["ArrowUp", { type: "previous-cue" }],
  ["ArrowLeft", { type: "skip", seconds: -5 }],
  ["ArrowRight", { type: "skip", seconds: 5 }],
  ["[", { type: "change-rate", direction: -1 }],
  ["]", { type: "change-rate", direction: 1 }],
  ["c", { type: "capture-lookup" }],
  ["r", { type: "toggle-review" }],
]);

const REVIEW_SHORTCUTS = new Map([
  [" ", { type: "review-replay" }],
  ["p", { type: "review-replay" }],
  ["Enter", { type: "review-reveal" }],
  ["0", { type: "review-rate", rating: 0 }],
  ["1", { type: "review-rate", rating: 1 }],
  ["2", { type: "review-rate", rating: 2 }],
  ["3", { type: "review-rate", rating: 3 }],
  ["4", { type: "review-rate", rating: 4 }],
  ["5", { type: "review-rate", rating: 5 }],
  ["r", { type: "toggle-review" }],
]);

function normalizedKey(value) {
  return value.length === 1 && value !== " " ? value.toLowerCase() : value;
}

function isEditingOrCommandTarget(target) {
  if (!target || typeof target.closest !== "function") return false;
  if (target.closest(".transcript-token")) return false;
  return Boolean(target.isContentEditable || target.closest(
    "input, textarea, select, button, a[href], [role='button'], [contenteditable]:not([contenteditable='false'])",
  ));
}

function isLookupCaptureTarget(target) {
  if (!target || typeof target.closest !== "function") return false;
  if (target.closest(".transcript-token")) return true;
  if (!target.closest("#lookup-inspector")) return false;
  return !Boolean(target.isContentEditable || target.closest(
    "input, textarea, select, [contenteditable]:not([contenteditable='false'])",
  ));
}

function isReviewSelectionTarget(target) {
  if (!target || typeof target.closest !== "function") return false;
  return Boolean(target.isContentEditable || target.closest(
    "input, textarea, select, [contenteditable]:not([contenteditable='false'])",
  ));
}

export function resolvePlayerShortcut(event, { openDialog = null, canCaptureLookup = false } = {}) {
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return null;
  const key = normalizedKey(event.key);
  if (openDialog) {
    if (openDialog !== "review") return null;
    const reviewAction = REVIEW_SHORTCUTS.get(key) ?? null;
    if (!reviewAction || event.repeat) return null;
    if (reviewAction.type === "toggle-review") return reviewAction;
    if (isReviewSelectionTarget(event.target)) return null;
    if (["review-replay", "review-reveal"].includes(reviewAction.type)
      && [" ", "Enter"].includes(key) && isEditingOrCommandTarget(event.target)) return null;
    return reviewAction;
  }
  const action = SHORTCUTS.get(key) ?? null;
  if (!action) return null;
  if (action.type === "capture-lookup") {
    if (!canCaptureLookup || event.repeat || openDialog) return null;
    return isLookupCaptureTarget(event.target) ? action : null;
  }
  if (event.repeat && ["toggle-playback", "toggle-review"].includes(action.type)) return null;
  return isEditingOrCommandTarget(event.target) ? null : action;
}

export function stepPlaybackRate(currentRate, direction) {
  const current = Number.isFinite(currentRate) ? currentRate : 1;
  if (direction > 0) return PLAYBACK_RATES.find((rate) => rate > current + Number.EPSILON) ?? PLAYBACK_RATES.at(-1);
  return PLAYBACK_RATES.findLast((rate) => rate < current - Number.EPSILON) ?? PLAYBACK_RATES[0];
}
