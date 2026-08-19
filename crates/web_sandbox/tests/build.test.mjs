import { describe, expect, test } from "bun:test";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { verifyDist } from "../scripts/verify-dist.mjs";

describe("offline sandbox distribution", () => {
  test("rejects cloud endpoints and missing precache entries", async () => {
    const dist = await mkdtemp(join(tmpdir(), "ensub-sandbox-dist-"));
    await mkdir(join(dist, "js"), { recursive: true });
    await writeFile(join(dist, "index.html"), "<script type=module src=\"./js/app.js\"></script>");
    await writeFile(join(dist, "js/app.js"), "fetch('https://example.invalid/api')");
    await writeFile(join(dist, "service-worker.js"), "const PRECACHE = ['./index.html'];");

    await expect(verifyDist(dist, { requiredFiles: [
      "index.html",
      "js/app.js",
      "service-worker.js",
    ] })).rejects.toThrow(/cross-origin|precache/i);
  });

  test("accepts a complete same-origin-only distribution", async () => {
    const dist = await mkdtemp(join(tmpdir(), "ensub-sandbox-dist-"));
    await mkdir(join(dist, "js"), { recursive: true });
    await writeFile(join(dist, "index.html"), "<script type=module src=\"./js/app.js\"></script>");
    await writeFile(join(dist, "js/app.js"), "export const ready = true;");
    await writeFile(join(dist, "service-worker.js"), [
      "const PRECACHE = [",
      "  \"./index.html\",",
      "  \"./js/app.js\",",
      "  \"./service-worker.js\"",
      "];",
    ].join("\n"));

    const result = await verifyDist(dist, { requiredFiles: [
      "index.html",
      "js/app.js",
      "service-worker.js",
    ] });
    expect(result.filesChecked).toBe(3);
  });
});
