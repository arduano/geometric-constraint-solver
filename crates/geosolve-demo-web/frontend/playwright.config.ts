// SPDX-License-Identifier: GPL-3.0-or-later
import { readFileSync } from "node:fs";
import { defineConfig, devices } from "@playwright/test";

const port = Number.parseInt(process.env.GEOSOLVE_E2E_PORT ?? "4173", 10);
const externalBaseUrl = process.env.GEOSOLVE_E2E_BASE_URL;
const preparedManifest = process.env.GEOSOLVE_E2E_ARTIFACT_MANIFEST;
if (externalBaseUrl && preparedManifest) throw new Error("choose an external URL or a prepared artifact, not both");
const prepared = preparedManifest ? JSON.parse(readFileSync(preparedManifest, "utf8")) : undefined;
if (prepared && (prepared.format !== "geosolve-release-artifact-v1" || prepared.kind !== "harness")) {
  throw new Error("full browser qualification requires a prepared compiler-harness artifact");
}
const prefix = prepared?.publicBase && prepared.publicBase !== "./" ? prepared.publicBase : "/";
// Fresh browser contexts isolate ordinary tests. Scale/atlas and legacy payload
// workflows share a single memory-heavy project slot under the gate's total cap.
const memoryHeavy = /M92 visual workflow .*: (perforated-fixture-field|robotic-harness-backplane|curves-contact-continuity-atlas|fabrication-operations-atlas)|legacy|scale|recovery/i;
const desktop = { ...devices["Desktop Chrome"], viewport: { width: 1024, height: 720 } };

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: Boolean(preparedManifest),
  timeout: 120_000,
  reporter: "list",
  use: {
    baseURL: externalBaseUrl ?? `http://127.0.0.1:${port}${prefix}`,
    trace: "retain-on-failure",
    launchOptions: process.env.GEOSOLVE_CHROMIUM_PATH ? { executablePath: process.env.GEOSOLVE_CHROMIUM_PATH } : undefined,
  },
  projects: preparedManifest ? [
    { name: "chromium", grepInvert: memoryHeavy, use: desktop },
    { name: "chromium-memory", grep: memoryHeavy, workers: 1, use: desktop },
  ] : [{ name: "chromium", use: desktop }],
  webServer: externalBaseUrl ? undefined : {
    command: preparedManifest ? "node scripts/serve-artifact.mjs" : `npm run wasm:release && GEOSOLVE_BROWSER_COMPILER_HARNESS=1 VITE_GEOSOLVE_MOCK=0 npm run build:ui && vite preview --host 127.0.0.1 --port ${port}`,
    port,
    reuseExistingServer: false,
    // A clean optimized WASM build can exceed three minutes on the release
    // qualification host. This timeout owns only fixture-server startup; the
    // per-test 120 second budget above remains unchanged.
    timeout: preparedManifest ? 30_000 : 600_000,
  },
});
