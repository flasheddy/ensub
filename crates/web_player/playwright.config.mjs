import { defineConfig } from "@playwright/test";
import { existsSync } from "node:fs";

const systemChromium = "/usr/bin/chromium";
const executablePath = process.env.CHROMIUM_PATH
  ?? (existsSync(systemChromium) ? systemChromium : undefined);
const port = process.env.PLAYER_TEST_PORT ?? "4175";
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: "./tests",
  testMatch: "player.spec.mjs",
  fullyParallel: false,
  workers: 1,
  timeout: 30_000,
  expect: { timeout: 8_000 },
  use: {
    baseURL,
    browserName: "chromium",
    headless: true,
    launchOptions: executablePath ? { executablePath, args: ["--no-sandbox"] } : {},
  },
  webServer: {
    command: `PORT=${port} bun scripts/serve.mjs`,
    url: baseURL,
    reuseExistingServer: false,
    timeout: 15_000,
  },
});
