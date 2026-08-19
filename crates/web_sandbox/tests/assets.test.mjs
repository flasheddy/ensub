import { describe, expect, test } from "bun:test";

import { loadLexicon } from "../js/assets.js";

const digest = new Uint8Array(32).fill(0xab);
const digestHex = Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("");

class PassthroughDecompressionStream {
  constructor(format) {
    expect(format).toBe("gzip");
    const transform = new TransformStream();
    this.readable = transform.readable;
    this.writable = transform.writable;
  }
}

describe("loadLexicon", () => {
  test("verifies the compressed asset before decompressing it", async () => {
    const requests = [];
    const bytes = new Uint8Array([1, 2, 3, 4]);
    const result = await loadLexicon({
      manifestUrl: "/assets/lexicon-v1.manifest.json",
      assetUrl: "/assets/lexicon-v1.postcard.gz",
      fetchImpl: async (url) => {
        requests.push(url);
        if (url.endsWith(".json")) {
          return new Response(JSON.stringify({ compressedSha256: digestHex }));
        }
        return new Response(bytes);
      },
      cryptoImpl: { subtle: { digest: async () => digest.buffer } },
      DecompressionStreamImpl: PassthroughDecompressionStream,
    });

    expect(Array.from(result)).toEqual(Array.from(bytes));
    expect(requests).toEqual([
      "/assets/lexicon-v1.manifest.json",
      "/assets/lexicon-v1.postcard.gz",
    ]);
  });

  test("rejects a digest mismatch with an actionable code", async () => {
    const bytes = new Uint8Array([1, 2, 3]);
    await expect(loadLexicon({
      manifestUrl: "/manifest.json",
      assetUrl: "/lexicon.gz",
      fetchImpl: async (url) => url.endsWith(".json")
        ? new Response(JSON.stringify({ compressedSha256: "00".repeat(32) }))
        : new Response(bytes),
      cryptoImpl: { subtle: { digest: async () => digest.buffer } },
      DecompressionStreamImpl: PassthroughDecompressionStream,
    })).rejects.toMatchObject({ code: "lexicon_integrity_failed" });
  });
});
