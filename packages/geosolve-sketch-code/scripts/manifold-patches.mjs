// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import ts from "typescript";
import { compilePatchArtifact } from "../dist/src/compiler.js";

/** Record the trusted bundled patch sources, retaining the historical M92 fixture. */
export async function manifoldPatches(directory, check) {
  const sdk = new URL("../dist/src/index.js", import.meta.url).href;
  for (const [stem, exportName] of [
    ["water-channel", "waterChannel"],
    ["point-to-point-channel", "pointToPointChannel"],
    ["silicone-groove", "siliconeGroove"],
  ]) {
    const source = await readFile(join(directory, "patches", `${stem}.patch.ts`), "utf8");
    const emitted = ts.transpileModule(source, {
      compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
      transformers: {
        before: [(context) => (file) => ts.visitNode(file, function visit(node) {
          if (ts.isImportDeclaration(node)) {
            assert.equal(node.moduleSpecifier.text, "@geosolve/sketch-code");
            return context.factory.updateImportDeclaration(
              node, node.modifiers, node.importClause,
              context.factory.createStringLiteral(sdk), node.attributes,
            );
          }
          return ts.visitEachChild(node, visit, context);
        })],
      },
    }).outputText;
    const module = await import(`data:text/javascript;base64,${Buffer.from(emitted).toString("base64")}`);
    const compiled = compilePatchArtifact({
      source,
      moduleSpecifier: `./patches/${stem}.patch.ts`,
      exportName,
      patch: module[exportName],
    });
    const path = join(directory, "patches", `${stem}.artifact.json`);
    if (check) {
      assert.equal(await readFile(path, "utf8"), compiled.canonicalJson,
        `${stem} patch artifact is stale; run generate:bundled-samples`);
    } else {
      await writeFile(path, compiled.canonicalJson);
    }
  }
}
