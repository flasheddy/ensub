import { defineConfig } from "@playwright/test";
import { existsSync } from "node:fs";

const systemChromium = "/usr/bin/chromium";
const executablePath = process.env.CHROMIUM_PATH
  ?? (existsSync(systemChromium) ? systemChromium : undefined);

export default defineConfig({
  testDir: "./tests",
  testMatch: "player.spec.mjs",
  fullyParallel: false,
  workers: 1,
  timeout: 30_000,
  expect: { timeout: 8_000 },
  use: {
    baseURL: "http://127.0.0.1:4175",
    browserName: "chromium",
    headless: true,
    launchOptions: executablePath ? { executablePath, args: ["--no-sandbox"] } : {},
  },
  webServer: {
    command: "bun scripts/serve.mjs",
    url: "http://127.0.0.1:4175",
    reuseExistingServer: false,
    timeout: 15_000,
  },
});
