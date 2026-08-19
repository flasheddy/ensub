export class SandboxRuntimeError extends Error {
  constructor(code, message, options = undefined) {
    super(message, options);
    this.name = "EnsubSandboxError";
    this.code = code;
  }
}

async function responseOrThrow(response, description) {
  if (!response?.ok) {
    throw new SandboxRuntimeError(
      "lexicon_fetch_failed",
      `Could not load ${description} (${response?.status ?? "no response"}).`,
    );
  }
  return response;
}

function hex(bytes) {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

export async function loadLexicon({
  manifestUrl,
  assetUrl,
  fetchImpl = globalThis.fetch,
  cryptoImpl = globalThis.crypto,
  DecompressionStreamImpl = globalThis.DecompressionStream,
}) {
  let manifestResponse;
  let assetResponse;
  try {
    [manifestResponse, assetResponse] = await Promise.all([
      fetchImpl(manifestUrl),
      fetchImpl(assetUrl),
    ]);
    await responseOrThrow(manifestResponse, "the lexicon manifest");
    await responseOrThrow(assetResponse, "the lexicon asset");
  } catch (error) {
    if (error?.code === "lexicon_fetch_failed") throw error;
    throw new SandboxRuntimeError(
      "lexicon_fetch_failed",
      "Could not load the offline lexicon.",
      { cause: error },
    );
  }

  const manifest = await manifestResponse.json();
  const compressed = new Uint8Array(await assetResponse.arrayBuffer());
  const digest = new Uint8Array(await cryptoImpl.subtle.digest("SHA-256", compressed));
  if (hex(digest) !== manifest.compressedSha256) {
    throw new SandboxRuntimeError(
      "lexicon_integrity_failed",
      "The offline lexicon failed its SHA-256 integrity check.",
    );
  }
  if (!DecompressionStreamImpl) {
    throw new SandboxRuntimeError(
      "lexicon_decompression_unavailable",
      "This browser cannot decompress the bundled offline lexicon.",
    );
  }

  try {
    const stream = new Blob([compressed])
      .stream()
      .pipeThrough(new DecompressionStreamImpl("gzip"));
    return new Uint8Array(await new Response(stream).arrayBuffer());
  } catch (error) {
    throw new SandboxRuntimeError(
      "lexicon_decompression_unavailable",
      "The bundled offline lexicon could not be decompressed.",
      { cause: error },
    );
  }
}
