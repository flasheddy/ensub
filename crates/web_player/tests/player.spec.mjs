import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await page.evaluate(async () => {
    indexedDB.deleteDatabase("ensub-player");
    localStorage.removeItem("ensub.sandbox.v1");
    const registrations = await navigator.serviceWorker.getRegistrations();
    await Promise.all(registrations.map((registration) => registration.unregister()));
  });
  await page.reload();
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
