const DEFAULT_TIMEOUT_MS = 15_000;
const DEFAULT_RESPONSE_LIMIT = 64 * 1024;
const LOOPBACK_HOSTS = new Set(["localhost", "127.0.0.1", "[::1]"]);

export class DisambiguationTransportError extends Error {
  constructor(code, message, options) {
    super(message, options);
    this.name = "EnsubDisambiguationTransportError";
    this.code = code;
  }
}

export function buildOpenAiChatCompletionsBody(prepared, model) {
  if (!prepared || typeof prepared.systemPrompt !== "string" || typeof prepared.userPrompt !== "string") {
    throw new DisambiguationTransportError("invalid_request", "Prepared disambiguation prompts are required.");
  }
  const normalizedModel = typeof model === "string" ? model.trim() : "";
  if (!normalizedModel) {
    throw new DisambiguationTransportError("invalid_request", "A provider model is required.");
  }
  return {
    model: normalizedModel,
    messages: [
      { role: "system", content: prepared.systemPrompt },
      { role: "user", content: prepared.userPrompt },
    ],
    response_format: { type: "json_object" },
  };
}

function validateEndpoint(value) {
  let endpoint;
  try {
    endpoint = new URL(value);
  } catch (cause) {
    throw new DisambiguationTransportError("invalid_endpoint", "Enter a valid provider endpoint URL.", { cause });
  }
  const loopbackHttp = endpoint.protocol === "http:" && LOOPBACK_HOSTS.has(endpoint.hostname);
  if ((endpoint.protocol !== "https:" && !loopbackHttp)
    || !endpoint.host || endpoint.username || endpoint.password || endpoint.hash) {
    throw new DisambiguationTransportError(
      "invalid_endpoint",
      "Provider endpoints must use HTTPS, except for local loopback development.",
    );
  }
  return endpoint;
}

async function readBoundedText(response, limit) {
  const declared = Number.parseInt(response.headers.get("content-length") ?? "", 10);
  if (Number.isFinite(declared) && declared > limit) {
    throw new DisambiguationTransportError("response_too_large", "The provider response is too large.");
  }
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.byteLength > limit) {
    throw new DisambiguationTransportError("response_too_large", "The provider response is too large.");
  }
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch (cause) {
    throw new DisambiguationTransportError("invalid_provider_response", "The provider response is not valid UTF-8.", { cause });
  }
}

export function createOpenAiChatCompletionsAdapter({
  fetchImpl = globalThis.fetch,
  timeoutMs = DEFAULT_TIMEOUT_MS,
  responseLimit = DEFAULT_RESPONSE_LIMIT,
} = {}) {
  return {
    id: "openai_chat_completions",
    async send({ endpointUrl, model, credential, prepared, signal }) {
      const endpoint = validateEndpoint(endpointUrl);
      const normalizedCredential = typeof credential === "string" ? credential.trim() : "";
      if (!normalizedCredential) {
        throw new DisambiguationTransportError("missing_credential", "A provider credential is required.");
      }
      const body = buildOpenAiChatCompletionsBody(prepared, model);
      const timeout = new AbortController();
      const timer = setTimeout(
        () => timeout.abort(new DOMException("Timed out", "TimeoutError")),
        timeoutMs,
      );
      const combined = signal && globalThis.AbortSignal?.any
        ? AbortSignal.any([signal, timeout.signal])
        : timeout.signal;
      try {
        let response;
        try {
          response = await fetchImpl(endpoint, {
            method: "POST",
            credentials: "omit",
            redirect: "error",
            cache: "no-store",
            signal: combined,
            headers: {
              "content-type": "application/json",
              authorization: `Bearer ${normalizedCredential}`,
            },
            body: JSON.stringify(body),
          });
        } catch (cause) {
          if (timeout.signal.aborted) {
            throw new DisambiguationTransportError("request_timeout", "The provider request timed out.", { cause });
          }
          if (signal?.aborted) throw cause;
          throw new DisambiguationTransportError(
            "browser_access_failed",
            "The browser could not access the provider endpoint.",
            { cause },
          );
        }
        if (!response.ok) {
          const code = response.status === 401 || response.status === 403
            ? "provider_auth_failed"
            : response.status === 429
              ? "provider_rate_limited"
              : "provider_http_error";
          throw new DisambiguationTransportError(code, `The provider returned HTTP ${response.status}.`);
        }
        const text = await readBoundedText(response, responseLimit);
        let envelope;
        try {
          envelope = JSON.parse(text);
        } catch (cause) {
          throw new DisambiguationTransportError(
            "invalid_provider_response",
            "The provider returned malformed JSON.",
            { cause },
          );
        }
        const content = envelope?.choices?.[0]?.message?.content;
        if (typeof content !== "string") {
          throw new DisambiguationTransportError(
            "invalid_provider_response",
            "The provider response did not contain message content.",
          );
        }
        return content;
      } finally {
        clearTimeout(timer);
      }
    },
  };
}
