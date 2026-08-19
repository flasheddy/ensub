import { createServer } from "node:http";

const port = Number.parseInt(process.env.FIXTURE_PORT ?? "4176", 10);
const identityLoads = new Map();

function feed({ guid = true } = {}) {
  const origin = `http://127.0.0.1:${port}`;
  return `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:podcast="https://podcastindex.org/namespace/1.0"><channel>
<title>Cross Origin Fixture</title><language>en</language><description>Synthetic browser fixture.</description>
<item>${guid ? '<guid isPermaLink="false">cross-origin-episode</guid>' : ""}<title>Cross Origin Episode</title>
<pubDate>Mon, 17 Aug 2026 08:00:00 +0000</pubDate>
<enclosure url="${origin}/missing-audio.wav" type="audio/wav" length="1" />
<podcast:transcript url="${origin}/transcript.vtt" type="text/vtt" language="en" rel="captions" />
</item></channel></rss>`;
}

function send(response, body, contentType, cors = true) {
  const headers = { "Content-Type": contentType, "Cache-Control": "no-store" };
  if (cors) headers["Access-Control-Allow-Origin"] = "*";
  response.writeHead(200, headers);
  response.end(body);
}

createServer((request, response) => {
  const url = new URL(request.url, `http://127.0.0.1:${port}`);
  if (url.pathname === "/feed.xml") return send(response, feed(), "application/rss+xml");
  if (url.pathname === "/no-cors.xml") return send(response, feed(), "application/rss+xml", false);
  if (url.pathname === "/identity.xml") {
    const key = url.search;
    const count = (identityLoads.get(key) ?? 0) + 1;
    identityLoads.set(key, count);
    return send(response, feed({ guid: count > 1 }), "application/rss+xml");
  }
  if (url.pathname === "/transcript.vtt") {
    return send(response, "WEBVTT\n\n00:00.000 --> 00:03.000\nListening crosses origins safely.\n", "text/vtt");
  }
  response.writeHead(404, { "Content-Type": "text/plain", "Access-Control-Allow-Origin": "*" });
  response.end("Not found");
}).listen(port, "127.0.0.1", () => console.log(`Ensub fixture: http://127.0.0.1:${port}`));
