import { afterAll, beforeAll, expect, test } from "bun:test";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { cp, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseByteRange } from "../scripts/http-range.mjs";

const playerRoot = dirname(dirname(fileURLToPath(import.meta.url)));
let previewProcess;
let previewRoot;
let previewUrl;
let demoAudio;

function waitForPreviewUrl(child) {
  return new Promise((resolve, reject) => {
    let stdout = "";
    let stderr = "";
    const timeout = setTimeout(() => reject(new Error(`preview server did not start: ${stderr}`)), 5_000);
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
      const match = stdout.match(/Ensub Player: (http:\/\/[^\s]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.once("exit", (code) => {
      clearTimeout(timeout);
      reject(new Error(`preview server exited with code ${code}: ${stderr}`));
    });
  });
}

beforeAll(async () => {
  previewRoot = await mkdtemp(join(tmpdir(), "ensub-player-preview-"));
  await mkdir(join(previewRoot, "scripts"), { recursive: true });
  await mkdir(join(previewRoot, "dist/assets"), { recursive: true });
  await cp(join(playerRoot, "scripts/serve.mjs"), join(previewRoot, "scripts/serve.mjs"));
  await cp(join(playerRoot, "scripts/http-range.mjs"), join(previewRoot, "scripts/http-range.mjs"));
  await cp(join(playerRoot, "assets/demo.mp3"), join(previewRoot, "dist/assets/demo.mp3"));
  demoAudio = await readFile(join(previewRoot, "dist/assets/demo.mp3"));
  previewProcess = spawn(process.execPath, [join(previewRoot, "scripts/serve.mjs")], {
    env: { ...process.env, PORT: "0" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  previewUrl = await waitForPreviewUrl(previewProcess);
});

afterAll(async () => {
  if (previewProcess?.exitCode === null) {
    previewProcess.kill("SIGTERM");
    await once(previewProcess, "exit");
  }
  if (previewRoot) await rm(previewRoot, { recursive: true, force: true });
});

test("parses closed and open-ended byte ranges", () => {
  expect(parseByteRange("bytes=100-199", 1_000)).toEqual({ start: 100, end: 199 });
  expect(parseByteRange("bytes=900-", 1_000)).toEqual({ start: 900, end: 999 });
  expect(parseByteRange("bytes=900-1500", 1_000)).toEqual({ start: 900, end: 999 });
});

test("parses suffix byte ranges from the end of the representation", () => {
  expect(parseByteRange("bytes=-200", 1_000)).toEqual({ start: 800, end: 999 });
  expect(parseByteRange("bytes=-2000", 1_000)).toEqual({ start: 0, end: 999 });
});

test("rejects unsupported or unsatisfiable byte ranges", () => {
  expect(parseByteRange("bytes=1000-", 1_000)).toBeNull();
  expect(parseByteRange("bytes=500-499", 1_000)).toBeNull();
  expect(parseByteRange("bytes=-0", 1_000)).toBeNull();
  expect(parseByteRange("bytes=0-1,4-5", 1_000)).toBeNull();
  expect(parseByteRange("items=0-1", 1_000)).toBeNull();
});

test("preview server returns the exact requested demo MP3 byte range", async () => {
  const start = 257;
  const end = 384;
  const response = await fetch(`${previewUrl}/assets/demo.mp3`, {
    headers: { Range: `bytes=${start}-${end}` },
  });

  expect(response.status).toBe(206);
  expect(response.headers.get("content-type")).toBe("audio/mpeg");
  expect(response.headers.get("content-range")).toBe(`bytes ${start}-${end}/${demoAudio.length}`);
  expect(response.headers.get("content-length")).toBe(String(end - start + 1));
  expect(Buffer.from(await response.arrayBuffer())).toEqual(demoAudio.subarray(start, end + 1));
});

test("preview server rejects an unsatisfiable demo MP3 byte range", async () => {
  const response = await fetch(`${previewUrl}/assets/demo.mp3`, {
    headers: { Range: `bytes=${demoAudio.length}-` },
  });

  expect(response.status).toBe(416);
  expect(response.headers.get("content-range")).toBe(`bytes */${demoAudio.length}`);
  expect((await response.arrayBuffer()).byteLength).toBe(0);
});
