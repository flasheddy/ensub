import { createReadStream, statSync } from "node:fs";
import { createServer } from "node:http";
import { dirname, extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(dirname(fileURLToPath(import.meta.url))), "dist");
const port = Number.parseInt(process.env.PORT ?? "4175", 10);
const types = {
  ".css": "text/css", ".gz": "application/gzip", ".html": "text/html", ".js": "text/javascript",
  ".json": "application/json", ".png": "image/png", ".wasm": "application/wasm",
  ".wav": "audio/wav", ".webmanifest": "application/manifest+json", ".xml": "application/rss+xml", ".vtt": "text/vtt",
};

createServer((request, response) => {
  const pathname = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
  const requested = normalize(pathname).replace(/^\/+/, "");
  let file = join(root, requested || "index.html");
  try {
    if (statSync(file).isDirectory()) file = join(file, "index.html");
    response.writeHead(200, { "Content-Type": types[extname(file)] ?? "application/octet-stream", "Cache-Control": "no-cache", "Accept-Ranges": "bytes" });
    createReadStream(file).pipe(response);
  } catch {
    response.writeHead(404, { "Content-Type": "text/plain" });
    response.end("Not found");
  }
}).listen(port, "127.0.0.1", () => console.log(`Ensub Player: http://127.0.0.1:${port}`));
