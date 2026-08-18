import { describe, expect, test } from "bun:test";
import { PLAYBACK_RATES, resolvePlayerShortcut, stepPlaybackRate } from "../js/shortcuts.js";

function target(kind = "workspace") {
  return {
    isContentEditable: kind === "contenteditable",
    closest(selector) {
      if (kind === "token" && selector.includes(".transcript-token")) return this;
      if (kind === "input" && selector.includes("input")) return this;
      if (kind === "select" && selector.includes("select")) return this;
      if (kind === "button" && selector.includes("button")) return this;
      if (kind === "lookup-button" && selector.includes("#lookup-inspector")) return this;
      if (kind === "lookup-button" && selector.includes("button")) return this;
      if (kind === "lookup-static" && selector.includes("#lookup-inspector")) return this;
      if (kind === "contenteditable" && selector.includes("[contenteditable")) return this;
      return null;
    },
  };
}

function key(keyValue, overrides = {}) {
  return {
    key: keyValue,
    target: target(),
    repeat: false,
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    ...overrides,
  };
}

describe("global player shortcuts", () => {
  test("maps every workspace command, including token-focused navigation", () => {
    expect(resolvePlayerShortcut(key(" "))).toEqual({ type: "toggle-playback" });
    expect(resolvePlayerShortcut(key("j"))).toEqual({ type: "next-cue" });
    expect(resolvePlayerShortcut(key("ArrowDown"))).toEqual({ type: "next-cue" });
    expect(resolvePlayerShortcut(key("K"))).toEqual({ type: "previous-cue" });
    expect(resolvePlayerShortcut(key("ArrowUp"))).toEqual({ type: "previous-cue" });
    expect(resolvePlayerShortcut(key("ArrowLeft"))).toEqual({ type: "skip", seconds: -5 });
    expect(resolvePlayerShortcut(key("ArrowRight", { target: target("token") }))).toEqual({ type: "skip", seconds: 5 });
    expect(resolvePlayerShortcut(key("["))).toEqual({ type: "change-rate", direction: -1 });
    expect(resolvePlayerShortcut(key("]"))).toEqual({ type: "change-rate", direction: 1 });
    expect(resolvePlayerShortcut(key("r"))).toEqual({ type: "toggle-review" });
  });

  test("leaves editable controls, ordinary buttons, modifiers, and unrelated keys alone", () => {
    for (const kind of ["input", "select", "contenteditable", "button"]) {
      expect(resolvePlayerShortcut(key(" ", { target: target(kind) }))).toBeNull();
    }
    expect(resolvePlayerShortcut(key("j", { ctrlKey: true }))).toBeNull();
    expect(resolvePlayerShortcut(key("ArrowRight", { altKey: true }))).toBeNull();
    expect(resolvePlayerShortcut(key("r", { metaKey: true }))).toBeNull();
    expect(resolvePlayerShortcut(key("j", { shiftKey: true }))).toBeNull();
    expect(resolvePlayerShortcut(key("Enter", { target: target("token") }))).toBeNull();
    expect(resolvePlayerShortcut(key("Tab", { target: target("token") }))).toBeNull();
  });

  test("C confirms only a capturable lookup from an allowed focus target", () => {
    const options = { canCaptureLookup: true };
    expect(resolvePlayerShortcut(key("c", { target: target("token") }), options))
      .toEqual({ type: "capture-lookup" });
    expect(resolvePlayerShortcut(key("C", { target: target("lookup-button") }), options))
      .toEqual({ type: "capture-lookup" });
    expect(resolvePlayerShortcut(key("c", { target: target("lookup-static") }), options))
      .toEqual({ type: "capture-lookup" });

    expect(resolvePlayerShortcut(key("c", { target: target("workspace") }), options)).toBeNull();
    expect(resolvePlayerShortcut(key("c", { target: target("input") }), options)).toBeNull();
    expect(resolvePlayerShortcut(key("c", { target: target("select") }), options)).toBeNull();
    expect(resolvePlayerShortcut(key("c", { target: target("contenteditable") }), options)).toBeNull();
    expect(resolvePlayerShortcut(key("c", { target: target("token"), repeat: true }), options)).toBeNull();
    expect(resolvePlayerShortcut(key("c", { target: target("token"), ctrlKey: true }), options)).toBeNull();
    expect(resolvePlayerShortcut(key("c", { target: target("token") }))).toBeNull();
    expect(resolvePlayerShortcut(key("c", { target: target("token") }), {
      canCaptureLookup: true,
      openDialog: "other",
    })).toBeNull();
  });

  test("only R escapes the review dialog and no shortcut escapes another dialog", () => {
    expect(resolvePlayerShortcut(key("r", { target: target("button") }), { openDialog: "review" }))
      .toEqual({ type: "toggle-review" });
    expect(resolvePlayerShortcut(key(" "), { openDialog: "review" })).toBeNull();
    expect(resolvePlayerShortcut(key("r"), { openDialog: "other" })).toBeNull();
  });

  test("repeat cannot retrigger toggles but can continue navigation", () => {
    expect(resolvePlayerShortcut(key(" ", { repeat: true }))).toBeNull();
    expect(resolvePlayerShortcut(key("r", { repeat: true }))).toBeNull();
    expect(resolvePlayerShortcut(key("ArrowRight", { repeat: true }))).toEqual({ type: "skip", seconds: 5 });
    expect(resolvePlayerShortcut(key("j", { repeat: true }))).toEqual({ type: "next-cue" });
  });
});

test("playback rates step through the fixed ladder and clamp at both ends", () => {
  expect(PLAYBACK_RATES).toEqual([0.75, 1, 1.25, 1.5, 1.75, 2]);
  expect(stepPlaybackRate(1, 1)).toBe(1.25);
  expect(stepPlaybackRate(1, -1)).toBe(0.75);
  expect(stepPlaybackRate(0.9, 1)).toBe(1);
  expect(stepPlaybackRate(1.6, -1)).toBe(1.5);
  expect(stepPlaybackRate(2, 1)).toBe(2);
  expect(stepPlaybackRate(0.75, -1)).toBe(0.75);
});
