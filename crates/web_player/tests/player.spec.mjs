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
  await token.press("ArrowRight");
  await expect(page.locator(".transcript-token").filter({ hasText: /^begins$/ }).first()).toBeFocused();
  await token.click();
  await expect(page.getByRole("complementary", { name: "Word lookup" })).toBeVisible();
  await expect(page.locator("#lookup-surface")).toHaveText("listening");
  await expect(page.locator(".lookup-definitions li").first()).toBeVisible();

  await page.getByRole("button", { name: "Capture", exact: true }).click();
  await expect(page.locator("#lookup-body")).toContainText("Saved as a new learning card");
  await page.getByRole("button", { name: "Close lookup" }).click();
  await token.click();
  await page.getByRole("button", { name: "Capture", exact: true }).click();
  await expect(page.locator("#lookup-body")).toContainText("already captured");

  await page.context().setOffline(true);
  await page.reload();
  await expect(page.locator(".cue")).toHaveCount(6);
  await page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).first().click();
  await expect(page.locator("#lookup-surface")).toHaveText("listening");
});

test("installed shell completes lookup capture and SRS review offline", async ({ page, context }) => {
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  await expect(page.locator(".cue")).toHaveCount(6);
  await page.evaluate(() => navigator.serviceWorker.ready);
  await context.setOffline(true);
  await page.reload();

  await expect(page.getByRole("heading", { name: "The Shape of a Listening Habit" })).toBeVisible();
  const token = page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).first();
  await token.click();
  await expect(page.locator("#lookup-surface")).toHaveText("listening");
  await page.getByRole("button", { name: "Capture", exact: true }).click();
  await expect(page.locator("#lookup-body")).toContainText("Saved as a new learning card");
  await page.getByRole("button", { name: /Review/ }).click();
  await page.getByRole("button", { name: "Reveal answer" }).click();
  await page.locator('.review-rating[data-rating="4"]').click();
  await page.getByRole("button", { name: "Next card" }).click();
  await expect(page.locator("#review-status")).toContainText("Review complete");
});

test("demo episode plays, syncs transcript, follows manually, and persists", async ({ page, context }) => {
  await expect(page.getByRole("button", { name: "Load Demo Episode" })).toBeVisible();
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  await expect(page.getByRole("heading", { name: "The Shape of a Listening Habit" })).toBeVisible();
  await expect(page.locator("#empty-state")).toBeHidden();
  await expect(page.locator("footer.player")).toBeVisible();
  await expect(page.locator(".cue")).toHaveCount(6);
  await expect(page.locator("audio")).toHaveAttribute("preload", "metadata");
  await expect.poll(() => page.locator("audio").evaluate((audio) => audio.duration)).toBeGreaterThan(20);

  await page.getByRole("button", { name: "Play" }).click();
  await expect(page.getByRole("button", { name: "Pause" })).toBeVisible();
  await page.locator("audio").evaluate((audio) => {
    audio.currentTime = 4;
    audio.dispatchEvent(new Event("timeupdate"));
  });
  await expect(page.locator(".cue").nth(1)).toHaveClass(/is-active/);

  await page.locator("#transcript").dispatchEvent("wheel");
  await expect(page.getByRole("button", { name: "Return to current cue" })).toBeVisible();
  await page.getByRole("button", { name: "Return to current cue" }).click();
  await expect(page.getByRole("button", { name: "Following" })).toBeVisible();

  await page.reload();
  await expect(page.getByRole("heading", { name: "The Shape of a Listening Habit" })).toBeVisible();
  await context.setOffline(true);
  await page.reload();
  await expect(page.locator(".cue")).toHaveCount(6);
});

test("mobile player has no horizontal overflow", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  await expect(page.locator(".cue")).toHaveCount(6);
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
    await expect(page.locator(".cue")).toHaveCount(6);
    expect(await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(0);
    await expect(page.locator("footer.player")).toBeInViewport();
  });
}

test("cue following scrolls on anchor changes and explicit resume only", async ({ page }) => {
  await page.evaluate(() => {
    window.__scrollCalls = [];
    Element.prototype.scrollIntoView = function scrollIntoView() { window.__scrollCalls.push(this.id); };
  });
  await page.getByRole("button", { name: "Load Demo Episode" }).click();
  await page.evaluate(() => { window.__scrollCalls = []; });
  await page.locator(".cue-seek").nth(1).click();
  await page.locator("audio").evaluate((audio) => audio.dispatchEvent(new Event("timeupdate")));
  await expect.poll(() => page.evaluate(() => window.__scrollCalls.length)).toBe(1);
  await page.locator("#transcript").dispatchEvent("wheel");
  await page.getByRole("button", { name: "Return to current cue" }).click();
  await expect.poll(() => page.evaluate(() => window.__scrollCalls.length)).toBe(2);
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
  await page.getByRole("button", { name: "Capture", exact: true }).click();
  await expect(page.locator("#lookup-body")).toContainText("Saved as a new learning card");

  await page.locator("#feed-url").fill(feedUrl);
  await page.getByRole("button", { name: "Load podcast feed" }).click();
  await expect(page.locator(".episode-row")).toHaveCount(1);
  await page.locator(".transcript-token").filter({ hasText: /^Listening$/ }).click();
  await page.getByRole("button", { name: "Capture", exact: true }).click();
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
  await page.getByRole("button", { name: "Capture", exact: true }).click();
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
  await page.getByRole("button", { name: "Capture", exact: true }).click();
  await expect(page.locator("#review-due-count")).toHaveText("1");

  await page.locator("audio").evaluate((audio) => { audio.currentTime = 4; audio.pause(); });
  await page.getByRole("button", { name: /Review/ }).click();
  await expect(page.getByRole("dialog", { name: "Review" })).toBeVisible();
  await expect(page.locator("#review-sentence")).toContainText("Listening begins");
  await expect(page.locator("#review-answer")).toBeHidden();
  await expect(page.locator(".review-rating").first()).toBeHidden();

  await page.getByRole("button", { name: "Replay snippet" }).click();
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
  expect(body.messages[1].content).not.toContain("audio.wav");
  expect(providerRequest.headers().authorization).toBe("Bearer synthetic-credential");
  await expect(page.locator(".lookup-definitions li").first()).toBeVisible();
});
