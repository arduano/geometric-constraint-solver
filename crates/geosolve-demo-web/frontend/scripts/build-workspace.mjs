// SPDX-License-Identifier: GPL-3.0-or-later
import { build } from "esbuild";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../../../../", import.meta.url));
await build({
  entryPoints: [new URL("workspace-runtime.ts", import.meta.url).pathname],
  outfile: `${root}/target/m98/workspace-runtime.mjs`,
  bundle: true, platform: "node", format: "esm", target: "node22",
  banner: { js: 'import { createRequire } from "node:module"; import { fileURLToPath } from "node:url"; import { dirname } from "node:path"; const require = createRequire(import.meta.url); const __filename = fileURLToPath(import.meta.url); const __dirname = dirname(__filename);' },
});
