export class PlayerTransportError extends Error {
  constructor(code, message, options) {
    super(message, options);
    this.name = "EnsubPlayerTransportError";
    this.code = code;
  }
}

export function validateRemoteUrl(value) {
  let url;
  try {
    url = new URL(value);
  } catch (cause) {
    throw new PlayerTransportError("invalid_url", "Enter a valid HTTP or HTTPS URL.", { cause });
  }
  if (!url.host || !["http:", "https:"].includes(url.protocol) || url.username || url.password) {
    throw new PlayerTransportError("invalid_url", "URL must use HTTP(S), include a host, and contain no credentials.");
  }
  return url;
}

export async function fetchBounded(value, {
  limit,
  signal,
  timeoutMs = 15_000,
  fetchImpl = globalThis.fetch,
} = {}) {
  const requestedUrl = validateRemoteUrl(value);
  const timeout = new AbortController();
  const timer = setTimeout(() => timeout.abort(new DOMException("Timed out", "TimeoutError")), timeoutMs);
  const combined = signal && globalThis.AbortSignal?.any
    ? AbortSignal.any([signal, timeout.signal])
    : timeout.signal;
  try {
    let response;
    try {
      response = await fetchImpl(requestedUrl, { signal: combined, redirect: "follow", credentials: "omit" });
    } catch (cause) {
      if (timeout.signal.aborted) {
        throw new PlayerTransportError("request_timeout", "The request timed out after 15 seconds.", { cause });
      }
      if (signal?.aborted) throw cause;
      if (cause instanceof TypeError) {
        throw new PlayerTransportError(
          "browser_access_failed",
          "The browser could not access this resource. CORS or browser network policy may be responsible.",
          { cause },
        );
      }
      throw cause;
    }
    validateRemoteUrl(response.url || requestedUrl.href);
    if (!response.ok) {
      throw new PlayerTransportError("http_error", `The server returned HTTP ${response.status}.`);
    }
    const declared = Number.parseInt(response.headers.get("content-length") ?? "", 10);
    if (Number.isFinite(declared) && declared > limit) {
      throw new PlayerTransportError("response_too_large", "The response exceeds the local player size limit.");
    }
    if (!response.body) {
      const bytes = new Uint8Array(await response.arrayBuffer());
      if (bytes.byteLength > limit) throw new PlayerTransportError("response_too_large", "The response exceeds the local player size limit.");
      return bytes;
    }
    const reader = response.body.getReader();
    const chunks = [];
    let total = 0;
    while (true) {
      const { done, value: chunk } = await reader.read();
      if (done) break;
      total += chunk.byteLength;
      if (total > limit) {
        await reader.cancel();
        throw new PlayerTransportError("response_too_large", "The response exceeds the local player size limit.");
      }
      chunks.push(chunk);
    }
    const output = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) {
      output.set(chunk, offset);
      offset += chunk.byteLength;
    }
    return output;
  } finally {
    clearTimeout(timer);
  }
}

export function decodeUtf8(bytes) {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch (cause) {
    throw new PlayerTransportError("invalid_utf8", "Transcript text must be valid UTF-8.", { cause });
  }
}
