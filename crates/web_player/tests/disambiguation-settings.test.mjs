import { expect, test } from "bun:test";
import {
  DISCLOSURE_VERSION,
  createDisambiguationSettings,
  normalizeProviderEndpoint,
} from "../js/disambiguation-settings.js";

class MemoryStorage {
  values = new Map();
  get length() { return this.values.size; }
  key(index) { return [...this.values.keys()][index] ?? null; }
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

test("clear removes Ensub provider configuration credentials and consent only", () => {
  const local = new MemoryStorage();
  const session = new MemoryStorage();
  local.setItem("unrelated", "preserved");
  session.setItem("unrelated", "preserved");
  const settings = createDisambiguationSettings({ local, session });
  settings.save({
    adapterId: "openai_chat_completions", endpointUrl: "https://provider.example.test/v1/chat/completions",
    model: "model", credential: "synthetic-secret", rememberCredential: true,
  });
  settings.grantConsent("openai_chat_completions", "https://provider.example.test/v1/chat/completions");

  settings.clear();

  expect(settings.load()).toBeNull();
  expect(settings.credential()).toBe("");
  expect(local.getItem("unrelated")).toBe("preserved");
  expect(session.getItem("unrelated")).toBe("preserved");
  expect([...local.values.keys()].some((key) => key.startsWith("ensub.disambiguation."))).toBe(false);
});
