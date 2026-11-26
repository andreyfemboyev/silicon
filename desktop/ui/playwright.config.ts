import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./tests",
  use: {
    baseURL: "http://localhost:1420",
    launchOptions: process.env.SILICON_CHROMIUM
      ? { executablePath: process.env.SILICON_CHROMIUM }
      : undefined,
  },
  webServer: {
    command: "npm run dev",
    url: "http://localhost:1420",
    reuseExistingServer: !process.env.CI,
  },
});
