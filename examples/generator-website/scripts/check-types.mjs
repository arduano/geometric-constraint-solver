// SPDX-License-Identifier: GPL-3.0-or-later
import { dirname, resolve } from "node:path";
import { engineEntry, folder, repository, sdkEntry } from "./build.mjs";

let typescript;
try { typescript = await import("typescript"); }
catch { typescript = await import(resolve(repository, "packages/geosolve-sketch-code/node_modules/typescript/lib/typescript.js")); }
const ts = typescript.default;
const program = ts.createProgram(["generator.ts", "protocol.ts", "worker.ts", "main.ts"].map((name) => resolve(folder, "src", name)), {
  noEmit: true, strict: true, skipLibCheck: true, target: ts.ScriptTarget.ES2022,
  module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler,
  lib: ["lib.es2022.d.ts", "lib.dom.d.ts"],
  paths: { "@geosolve/engine": [resolve(dirname(engineEntry), "index.d.ts")], "@geosolve/sketch-code": [resolve(dirname(sdkEntry), "index.d.ts")] },
});
const diagnostics = ts.getPreEmitDiagnostics(program);
if (diagnostics.length) throw Error(ts.formatDiagnosticsWithColorAndContext(diagnostics, {
  getCurrentDirectory: () => folder, getCanonicalFileName: (path) => path, getNewLine: () => "\n",
}));
console.log("Generator website TypeScript check passed");
