import { describe, expect, test } from "bun:test";
import { PlayerTransportError, fetchBounded, validateRemoteUrl } from "../js/transport.js";

describe("bounded browser transport", () => {
  test("accepts only credential-free HTTP(S) URLs with hosts", () => {
    expect(validateRemoteUrl("https://media.example.test/feed.xml").href).toBe("https://media.example.test/feed.xml");
    for (const value of ["file:///tmp/feed.xml", "https://user:secret@example.test/feed", "https://", "not a url"]) {
      expect(() => validateRemoteUrl(value)).toThrow(PlayerTransportError);
    }
  });

  test("enforces streamed limits without trusting content-length", async () => {
    const chunks = [new Uint8Array([1, 2, 3]), new Uint8Array([4, 5, 6])];
    const fetchImpl = async () => new Response(new ReadableStream({
      pull(controller) {
        const chunk = chunks.shift();
        if (chunk) controller.enqueue(chunk);
        else controller.close();
      },
    }), { status: 200 });
    await expect(fetchBounded("https://media.example.test/data", { limit: 5, fetchImpl })).rejects.toMatchObject({ code: "response_too_large" });
  });

  test("reports generic fetch failures without claiming a CORS diagnosis", async () => {
    const fetchImpl = async () => { throw new TypeError("Failed to fetch"); };
    await expect(fetchBounded("https://media.example.test/feed", { limit: 10, fetchImpl })).rejects.toMatchObject({
      code: "browser_access_failed",
      message: expect.stringContaining("CORS or browser network policy may be responsible"),
    });
  });
});
