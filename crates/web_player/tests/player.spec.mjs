import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await page.evaluate(async () => {
    indexedDB.deleteDatabase("ensub-player");
    const registrations = await navigator.serviceWorker.getRegistrations();
    await Promise.all(registrations.map((registration) => registration.unregister()));
  });
  await page.reload();
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
