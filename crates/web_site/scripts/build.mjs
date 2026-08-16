import { cp, mkdir, rm } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const dist = join(root, "dist");
await rm(dist, { recursive: true, force: true });
await mkdir(dist, { recursive: true });
for (const item of ["index.html", "styles.css", "service-worker.js", "vercel.json", "js"]) {
  await cp(join(root, item), join(dist, item), { recursive: true });
}
console.log(`Built static site at ${dist}`);
