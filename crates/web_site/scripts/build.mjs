import { execFile } from "node:child_process";
import { cp, mkdir, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const workspaceRoot = dirname(dirname(root));
const dist = join(root, "dist");
const run = promisify(execFile);

const { stdout: themeCss } = await run(
  "cargo",
  ["run", "--locked", "--quiet", "-p", "ensub-theme", "--bin", "ensub-theme-css"],
  { cwd: workspaceRoot },
);
if (!themeCss.trim()) throw new Error("ensub-theme-css produced an empty stylesheet");

await rm(dist, { recursive: true, force: true });
await mkdir(dist, { recursive: true });
for (const item of ["index.html", "styles.css", "service-worker.js", "js"]) {
  await cp(join(root, item), join(dist, item), { recursive: true });
}
await writeFile(join(dist, "theme.css"), themeCss, "utf8");
console.log(`Built static site at ${dist}`);
