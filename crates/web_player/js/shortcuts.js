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

export function resolvePlayerShortcut(event, { openDialog = null } = {}) {
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return null;
  const key = normalizedKey(event.key);
  const action = SHORTCUTS.get(key) ?? null;
  if (!action) return null;
  if (event.repeat && ["toggle-playback", "toggle-review"].includes(action.type)) return null;
  if (openDialog) return openDialog === "review" && action.type === "toggle-review" ? action : null;
  return isEditingOrCommandTarget(event.target) ? null : action;
}

export function stepPlaybackRate(currentRate, direction) {
  const current = Number.isFinite(currentRate) ? currentRate : 1;
  if (direction > 0) return PLAYBACK_RATES.find((rate) => rate > current + Number.EPSILON) ?? PLAYBACK_RATES.at(-1);
  return PLAYBACK_RATES.findLast((rate) => rate < current - Number.EPSILON) ?? PLAYBACK_RATES[0];
}
