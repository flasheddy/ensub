import { createReadStream, statSync } from "node:fs";
import { createServer } from "node:http";
import { dirname, extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { parseByteRange } from "./http-range.mjs";

const root = join(dirname(dirname(fileURLToPath(import.meta.url))), "dist");
const port = Number.parseInt(process.env.PORT ?? "4175", 10);
const types = {
  ".css": "text/css", ".gz": "application/gzip", ".html": "text/html", ".js": "text/javascript",
  ".json": "application/json", ".mp3": "audio/mpeg", ".png": "image/png", ".wasm": "application/wasm",
  ".wav": "audio/wav", ".webmanifest": "application/manifest+json", ".xml": "application/rss+xml", ".vtt": "text/vtt",
};

const server = createServer((request, response) => {
  const pathname = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
  const requested = normalize(pathname).replace(/^\/+/, "");
  let file = join(root, requested || "index.html");
  try {
    if (statSync(file).isDirectory()) file = join(file, "index.html");
    const size = statSync(file).size;
    const headers = {
      "Content-Type": types[extname(file)] ?? "application/octet-stream",
      "Cache-Control": "no-cache",
      "Accept-Ranges": "bytes",
    };
    if (request.headers.range !== undefined) {
      const range = parseByteRange(request.headers.range, size);
      if (!range) {
        response.writeHead(416, { ...headers, "Content-Range": `bytes */${size}` });
        response.end();
        return;
      }
      const { start, end } = range;
      response.writeHead(206, {
        ...headers,
        "Content-Length": String(end - start + 1),
        "Content-Range": `bytes ${start}-${end}/${size}`,
      });
      createReadStream(file, { start, end }).pipe(response);
      return;
    }
    response.writeHead(200, { ...headers, "Content-Length": String(size) });
    createReadStream(file).pipe(response);
  } catch {
    response.writeHead(404, { "Content-Type": "text/plain" });
    response.end("Not found");
  }
});

server.listen(port, "127.0.0.1", () => {
  const address = server.address();
  const boundPort = typeof address === "object" && address ? address.port : port;
  console.log(`Ensub Player: http://127.0.0.1:${boundPort}`);
});
