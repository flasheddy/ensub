import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const fixture = await readFile(
  join(dirname(fileURLToPath(import.meta.url)), "fixtures/performance-paragraph.txt"),
  "utf8",
);

async function ready(page) {
  await page.goto("/");
  await page.waitForFunction(() => Boolean(globalThis.__ensubSandbox));
}

async function clear(page) {
  await page.evaluate(async () => {
    await globalThis.__ensubSandbox.client.reset();
  });
}

async function parseAndCapture(page, text, lemma) {
  return page.evaluate(async ({ text, lemma }) => {
    const { client } = globalThis.__ensubSandbox;
    const parsed = client.parse({ text, includeStopwords: false, maxCandidates: 100 });
    const candidate = parsed.candidates.find((value) => value.lemma === lemma);
    if (!candidate) throw new Error(`fixture did not resolve ${lemma}`);
    return client.captureParsed({
      text,
      candidateIds: [candidate.id],
      source: "playwright:synthetic",
      capturedAtMs: Date.now(),
      includeStopwords: false,
      maxCandidates: 100,
    });
  }, { text, lemma });
}

test.beforeEach(async ({ page }) => {
  await ready(page);
  await clear(page);
});

test("real lexicon parser p95 remains below 100 ms", async ({ page }) => {
  const measurements = await page.evaluate((text) => {
    const input = { text, includeStopwords: false, maxCandidates: 100 };
    globalThis.__ensubSandbox.client.parse(input);
    const values = [];
    for (let index = 0; index < 30; index += 1) {
      const started = performance.now();
      const output = globalThis.__ensubSandbox.client.parse(input);
      if (output.candidates.length === 0) throw new Error("real lexicon returned no candidates");
      values.push(performance.now() - started);
    }
    return values;
  }, fixture);
  measurements.sort((left, right) => left - right);
  const p95 = measurements[Math.ceil(measurements.length * 0.95) - 1];
  expect(p95, `parser p95 was ${p95.toFixed(2)} ms`).toBeLessThan(100);
});

test("capture survives a full reload and rating 4 advances SRS state", async ({ page }) => {
  await parseAndCapture(page, "Immersion makes language memorable.", "immersion");
  await page.reload();
  await page.waitForFunction(() => Boolean(globalThis.__ensubSandbox));
  await expect(page.getByTestId("stats")).toHaveText("1 cards | 1 due");
  await page.getByRole("button", { name: "Reveal" }).click();
  await expect(page.getByTestId("answer")).toBeVisible();
  await page.locator('[data-action="rate"][data-rating="4"]').click();
  await expect(page.getByTestId("stats")).toHaveText("1 cards | 0 due");
  const stats = await page.evaluate(() => globalThis.__ensubSandbox.client.stats({ asOfMs: Date.now() }));
  expect(stats.intervals.days1To6).toBe(1);
});

test("two tabs retain serialized captures and reject a stale review", async ({ context, page }) => {
  const second = await context.newPage();
  await ready(second);
  await Promise.all([
    parseAndCapture(page, "Immersion supports learning.", "immersion"),
    parseAndCapture(second, "Vocabulary supports reading.", "vocabulary"),
  ]);
  const stats = await page.evaluate(() => globalThis.__ensubSandbox.client.stats({ asOfMs: Date.now() }));
  expect(stats.totalCards).toBe(2);

  const [firstCard, staleCard] = await Promise.all([page, second].map((tab) => tab.evaluate(() => (
    globalThis.__ensubSandbox.client.dueReviews({ asOfMs: Date.now(), limit: 1 }).cards[0]
  ))));
  expect(firstCard.wordId).toBe(staleCard.wordId);
  const first = await page.evaluate((card) => globalThis.__ensubSandbox.client.review({
    wordId: card.wordId,
    reviewToken: card.reviewToken,
    rating: 4,
    reviewedAtMs: Date.now(),
  }), firstCard);
  expect(first.repetitions).toBe(1);
  const conflictCode = await second.evaluate(async (card) => {
    try {
      await globalThis.__ensubSandbox.client.review({
        wordId: card.wordId,
        reviewToken: card.reviewToken,
        rating: 4,
        reviewedAtMs: Date.now(),
      });
      return "missing_conflict";
    } catch (error) {
      return error.code;
    }
  }, staleCard);
  expect(conflictCode).toBe("review_conflict");
});

test("storage events refresh another tab", async ({ context, page }) => {
  const second = await context.newPage();
  await ready(second);
  await parseAndCapture(page, "Immersion supports learning.", "immersion");
  await expect(second.getByTestId("stats")).toHaveText("1 cards | 1 due");
});

test("corrupt snapshots remain untouched and surface their error code", async ({ page }) => {
  const corrupt = "{not-json";
  await page.evaluate((value) => localStorage.setItem("ensub.sandbox.v1", value), corrupt);
  await page.reload();
  await expect(page.getByTestId("status")).toHaveAttribute("data-error-code", "storage_corrupt");
  expect(await page.evaluate(() => localStorage.getItem("ensub.sandbox.v1"))).toBe(corrupt);
});

test("all requests are same-origin and a controlled offline reload remains functional", async ({ context, page }) => {
  const remote = [];
  page.on("request", (request) => {
    if (new URL(request.url()).origin !== "http://127.0.0.1:4174") remote.push(request.url());
  });
  await page.evaluate(() => navigator.serviceWorker.ready);
  await page.reload();
  await page.waitForFunction(() => navigator.serviceWorker.controller !== null);
  await context.setOffline(true);
  await page.reload();
  await page.waitForFunction(() => Boolean(globalThis.__ensubSandbox));
  await parseAndCapture(page, "Immersion makes language memorable.", "immersion");
  await page.evaluate(() => globalThis.__ensubSandbox.controller.refresh());
  await expect(page.getByTestId("stats")).toHaveText("1 cards | 1 due");
  expect(remote).toEqual([]);
});
