// SPDX-License-Identifier: GPL-3.0-or-later
import { build } from "esbuild";
import { fileURLToPath } from "node:url";

// The folder worker hosts the public engine and genuine managed compiler.
await build({
  entryPoints: [fileURLToPath(new URL("../src/workspace-runtime.ts", import.meta.url))],
  outfile: fileURLToPath(new URL("../dist/workspace-runtime.mjs", import.meta.url)),
  bundle: true, platform: "node", format: "esm", target: "node22",
  packages: "external",
  banner: { js: 'import { createRequire } from "node:module"; import { fileURLToPath } from "node:url"; import { dirname } from "node:path"; const require = createRequire(import.meta.url); const __filename = fileURLToPath(import.meta.url); const __dirname = dirname(__filename);' },
});
