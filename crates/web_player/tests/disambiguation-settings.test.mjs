import { expect, test } from "bun:test";
import {
  DISCLOSURE_VERSION,
  createDisambiguationSettings,
  normalizeProviderEndpoint,
} from "../js/disambiguation-settings.js";

class MemoryStorage {
  values = new Map();
  getItem(key) { return this.values.get(key) ?? null; }
  setItem(key, value) { this.values.set(key, String(value)); }
  removeItem(key) { this.values.delete(key); }
}

test("provider metadata is versioned and credentials default to session storage", () => {
  const local = new MemoryStorage();
  const session = new MemoryStorage();
  const settings = createDisambiguationSettings({ local, session });
  settings.save({
    adapterId: "openai_chat_completions",
    endpointUrl: "https://provider.example.test/v1/chat/completions",
    model: "model",
    credential: "synthetic-secret",
    rememberCredential: false,
  });

  expect(settings.load()).toMatchObject({ version: 1, model: "model", rememberCredential: false });
  expect(settings.credential()).toBe("synthetic-secret");
  expect(JSON.stringify([...local.values])).not.toContain("synthetic-secret");
  expect(JSON.stringify([...session.values])).toContain("synthetic-secret");
});

test("persistent credentials require an explicit remember choice", () => {
  const local = new MemoryStorage();
  const session = new MemoryStorage();
  const settings = createDisambiguationSettings({ local, session });
  settings.save({
    adapterId: "openai_chat_completions", endpointUrl: "https://provider.example.test/v1/chat/completions",
    model: "model", credential: "synthetic-secret", rememberCredential: true,
  });
  expect(settings.credential()).toBe("synthetic-secret");
  expect(JSON.stringify([...local.values])).toContain("synthetic-secret");
  expect(JSON.stringify([...session.values])).not.toContain("synthetic-secret");
});

test("consent is scoped to adapter endpoint and disclosure version", () => {
  const settings = createDisambiguationSettings({ local: new MemoryStorage(), session: new MemoryStorage() });
  const endpoint = normalizeProviderEndpoint("HTTPS://Provider.Example.test/v1/chat/completions");
  expect(settings.hasConsent("openai_chat_completions", endpoint)).toBe(false);
  settings.grantConsent("openai_chat_completions", endpoint);
  expect(settings.hasConsent("openai_chat_completions", endpoint)).toBe(true);
  expect(settings.consentRecord("openai_chat_completions", endpoint).disclosureVersion).toBe(DISCLOSURE_VERSION);
  expect(settings.hasConsent("openai_chat_completions", "https://other.example.test/v1/chat/completions")).toBe(false);
});
