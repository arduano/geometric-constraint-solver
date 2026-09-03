// SPDX-License-Identifier: GPL-3.0-or-later
import { defineConfig, devices } from "@playwright/test";

const port = Number.parseInt(process.env.GEOSOLVE_E2E_PORT ?? "4173", 10);
const externalBaseUrl = process.env.GEOSOLVE_E2E_BASE_URL;

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: false,
  timeout: 120_000,
  reporter: "list",
  use: {
    baseURL: externalBaseUrl ?? `http://127.0.0.1:${port}`,
    trace: "retain-on-failure",
    launchOptions: process.env.GEOSOLVE_CHROMIUM_PATH ? { executablePath: process.env.GEOSOLVE_CHROMIUM_PATH } : undefined,
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1024, height: 720 } } }],
  webServer: externalBaseUrl ? undefined : {
    command: `npm run wasm:release && GEOSOLVE_BROWSER_COMPILER_HARNESS=1 VITE_GEOSOLVE_MOCK=0 npm run build:ui && vite preview --host 127.0.0.1 --port ${port}`,
    port,
    reuseExistingServer: false,
    timeout: 180_000,
  },
});
