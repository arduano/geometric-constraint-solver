// SPDX-License-Identifier: GPL-3.0-or-later
import { build } from "../../../crates/geosolve-demo-web/frontend/node_modules/esbuild/lib/main.js";
import ts from "../../../packages/geosolve-sketch-code/node_modules/typescript/lib/typescript.js";
import { mkdirSync, copyFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
const root = fileURLToPath(new URL("..", import.meta.url));
mkdirSync(resolve(root, "dist"), { recursive: true });
await build({ entryPoints: [resolve(root, "src/index.ts"), resolve(root, "src/host.ts"), resolve(root, "src/client.ts")], outdir: resolve(root, "dist"), bundle: true, format: "esm", platform: "neutral", target: "es2022" });
const source = resolve(root, "src/index.ts");
const options = { declaration: true, emitDeclarationOnly: true, strict: true, target: ts.ScriptTarget.ES2022,
  module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler, skipLibCheck: true,
  outDir: resolve(root, "dist"), rootDir: resolve(root, "src"),
};
const program = ts.createProgram([source, resolve(root, "src/host.ts"), resolve(root, "src/client.ts")], options);
const result = program.emit();
const diagnostics = [...ts.getPreEmitDiagnostics(program), ...result.diagnostics];
if (diagnostics.length) throw Error(ts.formatDiagnosticsWithColorAndContext(diagnostics, { getCurrentDirectory: () => root, getCanonicalFileName: (path) => path, getNewLine: () => "\n" }));
const license = resolve(root, "../../LICENSE");
if (existsSync(license)) copyFileSync(license, resolve(root, "LICENSE"));
