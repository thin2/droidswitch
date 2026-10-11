import { defineConfig } from "@playwright/test";
const port = Number(process.env.PLAYWRIGHT_PORT || 1420);
if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error("Invalid PLAYWRIGHT_PORT");
export default defineConfig({
  testDir: "tests/ui",
  workers: 1,
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    browserName: "chromium",
    channel: process.env.PLAYWRIGHT_CHANNEL || "chrome",
  },
  webServer: {
    command: `npm run dev -- --port ${port}`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: !process.env.CI,
  },
  reporter: "list",
});
