import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";

test("player contains an accessible review dialog with prompt reveal and rating controls", async () => {
  const html = await readFile(new URL("../index.html", import.meta.url), "utf8");
  expect(html).toContain('id="review-button"');
  expect(html).toContain('id="review-dialog"');
  expect(html).toContain('role="dialog"');
  expect(html).toContain('aria-modal="true"');
  expect(html).toContain('id="review-headword"');
  expect(html).toContain('id="review-sentence"');
  expect(html).toContain('id="review-play"');
  expect(html).toContain('id="review-reveal"');
  expect(html).toContain('id="review-part-of-speech"');
  expect(html.match(/class="review-rating"/g)).toHaveLength(6);

  const css = await readFile(new URL("../styles.css", import.meta.url), "utf8");
  expect(css).toContain(".review-target");
  expect(css).toContain("background: #111118;");
});

test("lookup contains an explicit AI action plus settings and payload consent dialogs", async () => {
  const html = await readFile(new URL("../index.html", import.meta.url), "utf8");
  expect(html).toContain('id="disambiguate-button"');
  expect(html).toContain('id="provider-settings-dialog"');
  expect(html).toContain('id="provider-consent-dialog"');
  expect(html).toContain('id="provider-payload-preview"');
  expect(html).toContain('id="provider-request-destination"');
  expect(html).toContain('id="provider-request-authorization"');
  expect(html).toContain("Allow and send");
  expect(html).toContain("AI-generated context");
  expect(html).toContain("selected word, saved sentence, candidate local dictionary senses, and episode label");
  expect(html).toContain('id="provider-remember"');
});
