import { describe, expect, test } from "bun:test";
import {
  DisambiguationTransportError,
  buildOpenAiChatCompletionsBody,
  createOpenAiChatCompletionsAdapter,
} from "../js/disambiguation-adapter.js";

const prepared = {
  systemPrompt: "STATIC PROMPT WITH EXPLICIT JSON SCHEMA",
  userPrompt: "Disambiguate this JSON-encoded untrusted lexical data:\n{\"selectedWord\":\"went\"}",
};

describe("OpenAI-compatible disambiguation adapter", () => {
  test("builds the exact schema-enforced chat completions body", () => {
    expect(buildOpenAiChatCompletionsBody(prepared, "fixture-model")).toEqual({
      model: "fixture-model",
      messages: [
        { role: "system", content: prepared.systemPrompt },
        { role: "user", content: prepared.userPrompt },
      ],
      response_format: { type: "json_object" },
    });
  });

  test("posts credentials only in the authorization header and returns raw content", async () => {
    let request;
    const adapter = createOpenAiChatCompletionsAdapter({
      fetchImpl: async (url, options) => {
        request = { url: url.href, options };
        return new Response(JSON.stringify({
          choices: [{ message: { content: "{\"matchedSenseId\":null,\"explanation\":\"Ambiguous.\",\"confidence\":\"low\"}" } }],
        }), { status: 200, headers: { "content-type": "application/json" } });
      },
    });

    const content = await adapter.send({
      endpointUrl: "https://provider.example.test/v1/chat/completions",
      model: "fixture-model",
      credential: "fixture-credential",
      prepared,
    });

    expect(content).toContain("matchedSenseId");
    expect(request.url).toBe("https://provider.example.test/v1/chat/completions");
    expect(request.options).toMatchObject({
      method: "POST",
      credentials: "omit",
      redirect: "error",
      cache: "no-store",
      headers: {
        "content-type": "application/json",
        authorization: "Bearer fixture-credential",
      },
    });
    expect(JSON.parse(request.options.body)).toEqual(
      buildOpenAiChatCompletionsBody(prepared, "fixture-model"),
    );
    expect(request.options.body).not.toContain("fixture-credential");
  });

  test("allows insecure HTTP only for loopback endpoints", async () => {
    const adapter = createOpenAiChatCompletionsAdapter({
      fetchImpl: async () => new Response(JSON.stringify({
        choices: [{ message: { content: "{}" } }],
      }), { status: 200 }),
    });

    await expect(adapter.send({
      endpointUrl: "http://provider.example.test/v1/chat/completions",
      model: "model",
      credential: "credential",
      prepared,
    })).rejects.toBeInstanceOf(DisambiguationTransportError);
    await expect(adapter.send({
      endpointUrl: "http://127.0.0.1:8787/v1/chat/completions",
      model: "model",
      credential: "credential",
      prepared,
    })).resolves.toBe("{}");
  });

  test("rejects malformed provider envelopes before Rust content validation", async () => {
    const responses = [
      {},
      { choices: [] },
      { choices: [{ message: {} }] },
      { choices: [{ message: { content: 7 } }] },
    ];
    for (const envelope of responses) {
      const adapter = createOpenAiChatCompletionsAdapter({
        fetchImpl: async () => new Response(JSON.stringify(envelope), { status: 200 }),
      });
      await expect(adapter.send({
        endpointUrl: "https://provider.example.test/v1/chat/completions",
        model: "model",
        credential: "credential",
        prepared,
      })).rejects.toMatchObject({ code: "invalid_provider_response" });
    }
  });
});
