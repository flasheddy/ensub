import { expect, test } from "@playwright/test";

const fixtureOrigin = `http://127.0.0.1:${process.env.PLAYER_FIXTURE_PORT ?? "4176"}`;

test.beforeEach(async ({ page }) => {
  await page.goto("/");
});

test.afterEach(async ({ context }) => {
  await context.setOffline(false);
});

test("token lookup is offline, keyboard reachable, and capture is idempotent", async ({ page }) => {
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  await expect(page.locator(".transcript-token")).not.toHaveCount(0);
  await expect(page.locator(".cue-copy").first()).toHaveText("Listening begins before we decide to study.");

  const token = page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).first();
  await token.focus();
  const nextToken = page.locator(".transcript-token").filter({ hasText: /^begins$/ }).first();
  await token.press("Tab");
  await expect(nextToken).toBeFocused();
  await nextToken.press("Shift+Tab");
  await expect(token).toBeFocused();
  await token.press("Enter");
  await expect(page.getByRole("complementary", { name: "Word lookup" })).toBeVisible();
  await expect(page.locator("#lookup-surface")).toHaveText("listening");
  await expect(page.locator(".lookup-definitions li").first()).toBeVisible();

  await token.press("c");
  await expect(page.locator("#lookup-body")).toContainText("Saved as a new learning card");
  await page.getByRole("button", { name: "Close lookup" }).click();

  const lastTokenInFirstCue = page.locator(".cue").first().locator(".transcript-token").last();
  const secondCueTimestamp = page.locator(".cue").nth(1).locator(".cue-seek");
  const firstTokenInSecondCue = page.locator(".cue").nth(1).locator(".transcript-token").first();
  await lastTokenInFirstCue.focus();
  await lastTokenInFirstCue.press("Tab");
  await expect(secondCueTimestamp).toBeFocused();
  await secondCueTimestamp.press("Tab");
  await expect(firstTokenInSecondCue).toBeFocused();
  await firstTokenInSecondCue.press("Shift+Tab");
  await expect(secondCueTimestamp).toBeFocused();
  await secondCueTimestamp.press("Shift+Tab");
  await expect(lastTokenInFirstCue).toBeFocused();

  const laterCueToken = page.locator(".cue").nth(11).locator(".transcript-token").filter({ hasText: /^listening$/ });
  await laterCueToken.focus();
  await laterCueToken.press("Enter");
  await expect(page.getByRole("complementary", { name: "Word lookup" })).toBeVisible();
  await expect(page.locator("#lookup-surface")).toHaveText("listening");
  await page.getByRole("button", { name: "Close lookup" }).click();

  await token.click();
  await page.getByRole("complementary", { name: "Word lookup" }).getByRole("button", { name: "Capture", exact: true }).click();
  await expect(page.locator("#lookup-body")).toContainText("already captured");

  await page.context().setOffline(true);
  await page.reload();
  await expect(page.locator(".cue")).toHaveCount(12);
  await page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).first().click();
  await expect(page.locator("#lookup-surface")).toHaveText("listening");
});

test("installed shell completes lookup capture and SRS review offline", async ({ page, context }) => {
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  await expect(page.locator(".cue")).toHaveCount(12);
  await page.evaluate(() => navigator.serviceWorker.ready);
  await context.setOffline(true);
  await page.reload();

  await expect(page.getByRole("heading", { name: "The Shape of a Listening Habit" })).toBeVisible();
  const token = page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).first();
  await token.click();
  await expect(page.locator("#lookup-surface")).toHaveText("listening");
  await page.getByRole("complementary", { name: "Word lookup" }).getByRole("button", { name: "Capture", exact: true }).click();
  await expect(page.locator("#lookup-body")).toContainText("Saved as a new learning card");
  await page.getByRole("button", { name: /Review/ }).click();
  await page.getByRole("button", { name: "Reveal answer" }).click();
  await page.locator('.review-rating[data-rating="4"]').click();
  await page.getByRole("button", { name: "Next card" }).click();
  await expect(page.locator("#review-status")).toContainText("Review complete");
});

test("clean install loads plays and seeks the precached demo offline", async ({ page, context }) => {
  await expect(page.locator(".episode-row")).toHaveCount(0);
  await expect(page.locator("#empty-state")).toBeVisible();
  await page.evaluate(() => navigator.serviceWorker.ready);
  await expect.poll(() => page.evaluate(() => navigator.serviceWorker.controller?.state)).toBe("activated");
  await expect.poll(() => page.evaluate(async () => {
    const url = new URL("./assets/demo.mp3", location.href).href;
    return (await caches.match(url))?.status ?? 0;
  })).toBe(200);

  await context.setOffline(true);
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  await expect(page.getByRole("heading", { name: "The Shape of a Listening Habit" })).toBeVisible();
  const audio = page.locator("audio");
  await expect(audio).toHaveAttribute("src", /demo\.mp3$/);
  await expect.poll(() => audio.evaluate((element) => element.duration)).toBeCloseTo(120, 2);
  await page.getByRole("button", { name: "Play" }).click();
  await expect.poll(() => audio.evaluate((element) => element.paused)).toBe(false);

  await page.locator(".cue-seek").nth(8).click();

  await expect.poll(() => audio.evaluate((element) => element.currentTime)).toBeGreaterThan(81.1);
  await expect.poll(() => audio.evaluate((element) => element.error)).toBeNull();
});

test("demo episode plays, syncs transcript, follows manually, and persists", async ({ page, context }) => {
  await expect(page.locator("#episode-workspace")).toBeVisible();
  await expect(page.locator("#empty-state")).toBeVisible();
  await expect(page.getByRole("button", { name: "Load Demo Episode" })).toBeVisible();
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  await expect(page.getByRole("heading", { name: "The Shape of a Listening Habit" })).toBeVisible();
  await expect(page.locator("#empty-state")).toBeHidden();
  await expect(page.getByRole("button", { name: "Load Demo Episode" })).toBeHidden();
  await expect(page.locator("footer.player")).toBeVisible();
  await expect(page.locator(".cue")).toHaveCount(12);
  await expect(page.locator("audio")).toHaveAttribute("preload", "metadata");
  await expect.poll(() => page.locator("audio").evaluate((audio) => audio.duration)).toBeCloseTo(120, 2);

  await page.getByRole("button", { name: "Play" }).click();
  await expect(page.getByRole("button", { name: "Pause" })).toBeVisible();
  await page.locator("audio").evaluate((audio) => {
    audio.currentTime = 29.5;
    audio.dispatchEvent(new Event("timeupdate"));
  });
  await expect(page.locator(".cue").nth(2)).toHaveClass(/is-active/);
  await expect(page.locator(".cue").nth(3)).toHaveClass(/is-active/);
  await page.locator("audio").evaluate((audio) => {
    audio.currentTime = 43;
    audio.dispatchEvent(new Event("seeked"));
  });
  await expect(page.locator(".cue").nth(4)).toHaveClass(/is-active/);

  await page.locator("#reader").dispatchEvent("wheel");
  await expect(page.getByRole("button", { name: "Return to active cue" })).toBeVisible();
  await page.getByRole("button", { name: "Return to active cue" }).click();
  await expect(page.getByRole("button", { name: "Return to active cue" })).toBeHidden();

  await page.reload();
  await expect(page.getByRole("heading", { name: "The Shape of a Listening Habit" })).toBeVisible();
  await context.setOffline(true);
  await page.reload();
  await expect(page.locator(".cue")).toHaveCount(12);
});

test("mobile player has no horizontal overflow", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  await expect(page.locator(".cue")).toHaveCount(12);
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBeLessThanOrEqual(0);
  await expect(page.locator("footer.player")).toBeVisible();
});

for (const viewport of [
  { width: 375, height: 667 }, { width: 375, height: 812 }, { width: 800, height: 900 },
  { width: 1024, height: 768 }, { width: 1440, height: 900 },
]) {
  test(`layout has no horizontal overflow at ${viewport.width}x${viewport.height}`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await page.getByRole("button", { name: "Load Demo Episode" }).click();
    await expect(page.locator(".cue")).toHaveCount(12);
    expect(await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(0);
    await expect(page.locator("footer.player")).toBeInViewport();
  });
}

test("cue following scrolls on anchor changes and explicit resume only", async ({ page }) => {
  await page.evaluate(() => {
    window.__scrollCalls = [];
    Element.prototype.scrollIntoView = function scrollIntoView() {
      window.__scrollCalls.push(this.id);
      document.querySelector("#reader").dispatchEvent(new Event("scroll"));
    };
  });
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  await expect.poll(() => page.evaluate(() => window.__scrollCalls.includes("cue-0"))).toBe(true);
  await page.evaluate(() => { window.__scrollCalls = []; });
  await page.locator(".cue-seek").nth(1).click();
  await page.locator("audio").evaluate((audio) => audio.dispatchEvent(new Event("timeupdate")));
  await expect.poll(() => page.evaluate(() => window.__scrollCalls)).toEqual(["cue-1"]);
  await expect(page.getByRole("button", { name: "Return to active cue" })).toBeHidden();
  await page.locator("#reader").dispatchEvent("wheel");
  await expect(page.getByRole("button", { name: "Return to active cue" })).toBeVisible();
  await page.getByRole("button", { name: "Return to active cue" }).click();
  await expect.poll(() => page.evaluate(() => window.__scrollCalls.length)).toBe(2);
});

test("keyboard scrolling immediately after returning suspends cue following", async ({ page }) => {
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  const reader = page.locator("#reader");
  const follow = page.getByRole("button", { name: "Return to active cue" });
  await reader.dispatchEvent("wheel");
  await expect(follow).toBeVisible();
  await follow.click();
  await expect(follow).toBeHidden();
  const initialScrollTop = await reader.evaluate((element) => element.scrollTop);

  await page.locator("#transcript").press("End");

  await expect.poll(() => reader.evaluate((element) => element.scrollTop)).toBeGreaterThan(initialScrollTop);
  await expect(follow).toBeVisible();
});

test("cue rows seek while token lookup and selected transcript text do not", async ({ page }) => {
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  const audio = page.locator("audio");
  await page.locator(".cue").nth(4).click({ position: { x: 3, y: 3 } });
  await expect.poll(() => audio.evaluate((element) => element.currentTime)).toBeCloseTo(42, 2);

  await audio.evaluate((element) => { element.currentTime = 7; });
  await page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).first().click();
  await expect(page.getByRole("complementary", { name: "Word lookup" })).toBeVisible();
  await expect.poll(() => audio.evaluate((element) => element.currentTime)).toBeCloseTo(7, 2);
  await page.getByRole("button", { name: "Close lookup" }).click();

  await page.evaluate(() => {
    const copy = document.querySelectorAll(".cue-copy")[5];
    const range = document.createRange();
    range.selectNodeContents(copy);
    const selection = getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    copy.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
  await expect.poll(() => audio.evaluate((element) => element.currentTime)).toBeCloseTo(7, 2);
});

test("retained cue text selection does not block timestamp seeking", async ({ page }) => {
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  const audio = page.locator("audio");
  const cueIndex = 4;
  await page.locator(".cue-copy").nth(cueIndex).evaluate((copy) => {
    const range = document.createRange();
    range.selectNodeContents(copy);
    const selection = getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
  });
  await audio.evaluate((element) => { element.currentTime = 7; });

  await page.locator(".cue-seek").nth(cueIndex).click();

  await expect.poll(() => audio.evaluate((element) => element.currentTime)).toBeCloseTo(42, 2);
});

test("review shortcut does not close Review behind Local Data", async ({ page }) => {
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  await page.keyboard.press("r");
  const review = page.getByRole("dialog", { name: "Review" });
  const localData = page.getByRole("dialog", { name: "Local data" });
  await expect(review).toBeVisible();
  await page.getByRole("button", { name: "Local data" }).click({ force: true });
  await expect(localData).toBeVisible();

  await page.keyboard.press("r");

  await expect(localData).toBeVisible();
  await expect(review).toBeVisible();
  await localData.getByRole("button", { name: "Close" }).focus();
  await page.keyboard.press("Escape");
  await expect(localData).toBeHidden();
  await expect(review).toBeVisible();
  await page.keyboard.press("r");
  await expect(review).toBeHidden();
});

test("global shortcuts control playback cues skips speed and review with the documented guards", async ({ page }) => {
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  const audio = page.locator("audio");
  const token = page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).first();
  await token.focus();

  await page.keyboard.press("Space");
  await expect(page.locator("#play-button")).toHaveAccessibleName("Pause");
  await page.keyboard.press("Space");
  await expect(page.locator("#play-button")).toHaveAccessibleName("Play");

  await audio.evaluate((element) => { element.currentTime = 10; });
  await page.keyboard.press("ArrowRight");
  await expect.poll(() => audio.evaluate((element) => element.currentTime)).toBeCloseTo(15, 1);
  await page.keyboard.press("ArrowLeft");
  await expect.poll(() => audio.evaluate((element) => element.currentTime)).toBeCloseTo(10, 1);

  await audio.evaluate((element) => { element.currentTime = 0; });
  await page.keyboard.press("j");
  await expect.poll(() => audio.evaluate((element) => element.currentTime)).toBeCloseTo(9, 2);
  await page.keyboard.press("ArrowDown");
  await expect.poll(() => audio.evaluate((element) => element.currentTime)).toBeCloseTo(19, 2);
  await page.keyboard.press("k");
  await expect.poll(() => audio.evaluate((element) => element.currentTime)).toBeCloseTo(9, 2);
  await page.keyboard.press("ArrowUp");
  await expect.poll(() => audio.evaluate((element) => element.currentTime)).toBeCloseTo(0, 2);

  await page.keyboard.press("]");
  await expect.poll(() => audio.evaluate((element) => element.playbackRate)).toBe(1.25);
  await expect(page.locator("#speed")).toHaveValue("1.25");
  await page.keyboard.press("[");
  await expect.poll(() => audio.evaluate((element) => element.playbackRate)).toBe(1);
  await expect(page.locator("#speed")).toHaveValue("1");

  await page.keyboard.press("r");
  const review = page.getByRole("dialog", { name: "Review" });
  await expect(review).toBeVisible();
  await page.getByRole("button", { name: "Exit review" }).focus();
  await page.keyboard.press("r");
  await expect(review).toBeHidden();

  await audio.evaluate((element) => { element.currentTime = 33; });
  await page.locator("#feed-url").focus();
  await page.keyboard.press("ArrowRight");
  await expect.poll(() => audio.evaluate((element) => element.currentTime)).toBeCloseTo(33, 2);
  await page.getByRole("button", { name: "Local data" }).click();
  await page.keyboard.press("r");
  await expect(page.getByRole("dialog", { name: "Local data" })).toBeVisible();
  await expect(review).toBeHidden();
});

test("cross-origin CORS feed loads and no-CORS failure is rendered", async ({ page }) => {
  await page.locator("#feed-url").fill(`${fixtureOrigin}/feed.xml`);
  await page.getByRole("button", { name: "Load podcast feed" }).click();
  await expect(page.getByRole("heading", { name: "Cross Origin Episode" })).toBeVisible();
  await expect(page.locator(".cue")).toHaveCount(1);

  await page.locator("#feed-url").fill(`${fixtureOrigin}/no-cors.xml`);
  await page.getByRole("button", { name: "Load podcast feed" }).click();
  await expect(page.locator("#feed-status")).toContainText("browser could not access");
});

test("a later publisher GUID preserves episode identity and captures", async ({ page }) => {
  const feedUrl = `${fixtureOrigin}/identity.xml?run=${Date.now()}`;
  await page.locator("#feed-url").fill(feedUrl);
  await page.getByRole("button", { name: "Load podcast feed" }).click();
  await expect(page.locator(".cue")).toHaveCount(1);
  await page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).click();
  await page.getByRole("complementary", { name: "Word lookup" }).getByRole("button", { name: "Capture", exact: true }).click();
  await expect(page.locator("#lookup-body")).toContainText("Saved as a new learning card");

  await page.locator("#feed-url").fill(feedUrl);
  await page.getByRole("button", { name: "Load podcast feed" }).click();
  await expect(page.locator(".episode-row")).toHaveCount(1);
  await page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).click();
  await page.getByRole("complementary", { name: "Word lookup" }).getByRole("button", { name: "Capture", exact: true }).click();
  await expect(page.locator("#lookup-body")).toContainText("already captured");
});

test("ordinary local workflows never contact a configured provider", async ({ page }) => {
  let providerCalls = 0;
  await page.route("**/provider", async (route) => { providerCalls += 1; await route.abort(); });
  await page.evaluate(() => {
    localStorage.setItem("ensub.disambiguation.config.v1", JSON.stringify({
      version: 1, adapterId: "openai_chat_completions", endpointUrl: `${location.origin}/provider`,
      model: "fixture-model", rememberCredential: true,
    }));
    localStorage.setItem("ensub.disambiguation.credential.local.v1", "synthetic-credential");
  });
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  await page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).first().click();
  await page.getByRole("complementary", { name: "Word lookup" }).getByRole("button", { name: "Capture", exact: true }).click();
  await page.getByRole("button", { name: /Review/ }).click();
  await page.getByRole("button", { name: "Reveal answer" }).click();
  await page.locator('.review-rating[data-rating="5"]').click();
  expect(providerCalls).toBe(0);
});

test("dialogs trap focus, close with Escape, and restore the invoking control", async ({ page }) => {
  await page.getByRole("button", { name: "Local data" }).focus();
  await page.getByRole("button", { name: "Local data" }).press("Enter");
  const dialog = page.getByRole("dialog", { name: "Local data" });
  await expect(dialog).toBeVisible();
  await expect(page.getByRole("button", { name: "Close" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(page.getByRole("button", { name: "Local data" })).toBeFocused();

  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  const token = page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).first();
  await token.click();
  await page.getByRole("button", { name: "Close lookup" }).click();
  await expect(token).toBeFocused();
});

test("captured cards review in-player with masked answers and restored playback", async ({ page }) => {
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  const token = page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).first();
  await token.click();
  await page.getByRole("complementary", { name: "Word lookup" }).getByRole("button", { name: "Capture", exact: true }).click();
  await expect(page.locator("#review-due-count")).toHaveText("1");

  await page.locator("audio").evaluate((audio) => { audio.currentTime = 4; audio.pause(); });
  await page.getByRole("button", { name: /Review/ }).click();
  await expect(page.getByRole("dialog", { name: "Review" })).toBeVisible();
  await expect(page.locator("#review-sentence")).toContainText("Listening begins");
  await expect(page.locator("#review-answer")).toBeHidden();
  await expect(page.locator(".review-rating").first()).toBeHidden();

  await page.getByRole("button", { name: "Replay snippet" }).click();
  await page.locator("audio").evaluate((audio) => {
    audio.currentTime = 119;
    audio.dispatchEvent(new Event("timeupdate"));
  });
  await expect(page.locator("#review-play")).toBeEnabled({ timeout: 5_000 });
  await expect(page.locator("#review-answer")).toBeHidden();
  await page.getByRole("button", { name: "Reveal answer" }).click();
  await expect(page.locator("#review-lemma")).toHaveText("listening");
  await expect(page.locator(".review-rating")).toHaveCount(6);
  await page.locator('.review-rating[data-rating="4"]').click();
  await expect(page.getByRole("button", { name: "Next card" })).toBeVisible();
  await page.getByRole("button", { name: "Next card" }).click();
  await expect(page.locator("#review-status")).toContainText("Review complete");
  await page.getByRole("button", { name: "Exit review" }).click();
  await expect(page.getByRole("dialog", { name: "Review" })).toBeHidden();
  await expect.poll(() => page.locator("audio").evaluate((audio) => audio.currentTime)).toBeCloseTo(4, 1);
  await expect(page.locator("#review-due-count")).toHaveText("0");
});

test("context explanation requires disclosure consent and sends the minimal schema-enforced payload", async ({ page }) => {
  let providerRequest;
  await page.route("**/provider", async (route) => {
    providerRequest = route.request();
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ choices: [{ message: { content: JSON.stringify({
        matchedSenseId: "sense-0-0", explanation: "The sentence uses listening as attentive hearing.", confidence: "high",
      }) } }] }),
    });
  });
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  await page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).first().click();
  await page.getByRole("button", { name: "Explain in context" }).click();
  await expect(page.getByRole("dialog", { name: "Context provider" })).toBeVisible();
  await page.locator("#provider-endpoint").fill(`${new URL(page.url()).origin}/provider`);
  await page.locator("#provider-model").fill("fixture-model");
  await page.locator("#provider-credential").fill("synthetic-credential");
  await page.getByRole("button", { name: "Save settings" }).click();

  await page.getByRole("button", { name: "Explain in context" }).click();
  const consent = page.getByRole("dialog", { name: "Send context to provider?" });
  await expect(consent).toBeVisible();
  const preview = JSON.parse(await page.locator("#provider-payload-preview").textContent());
  expect(Object.keys(preview).sort()).toEqual(["candidateSenses", "episodeLabel", "savedSentence", "selectedWord"]);
  expect(providerRequest).toBeUndefined();
  await page.getByRole("button", { name: "Send once" }).click();
  await expect(page.locator("#disambiguation-message")).toContainText("attentive hearing");

  const body = JSON.parse(providerRequest.postData());
  expect(body.response_format).toEqual({ type: "json_object" });
  expect(body.messages[0].content).toContain('"additionalProperties": false');
  expect(body.messages[1].content).not.toContain("demo.mp3");
  expect(providerRequest.headers().authorization).toBe("Bearer synthetic-credential");
  await expect(page.locator(".lookup-definitions li").first()).toBeVisible();
});
