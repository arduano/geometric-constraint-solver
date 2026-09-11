// SPDX-License-Identifier: GPL-3.0-or-later
import { readFileSync, writeFileSync } from "node:fs";
import { compileManagedSource, applyManagedSketchMutation } from "../../../../packages/geosolve-sketch-code/dist/src/managed.js";
if (process.argv.includes("--rejection")) {
  const source = readFileSync(new URL("tool-operation-rejection.ts", import.meta.url), "utf8");
  const normalized = compileManagedSource(source).normalizedSource;
  writeFileSync(new URL("tool-operation-rejection.json", import.meta.url), `${JSON.stringify(compileManagedSource(normalized), null, 2)}\n`);
} else if (process.argv.includes("--feature")) {
  const source = `"use geosolve sketch";\nimport { sketch } from "@geosolve/sketch-code";\nexport default sketch(($) => { const corner = $.geometry.polyline("corner", {vertices:[{key:"start",position:[200,0]},{key:"corner",position:[220,0]},{key:"end",position:[220,20]}]}); return {corner}; });\n`;
  const normalized=compileManagedSource(source).normalizedSource;
  writeFileSync(new URL("tool-operation-feature.json",import.meta.url),`${JSON.stringify(compileManagedSource(normalized),null,2)}\n`);
} else if (process.argv.includes("--basis")) {
  const source = readFileSync(new URL("tool-operation-basis.ts", import.meta.url), "utf8");
  const normalized = compileManagedSource(source).normalizedSource;
  writeFileSync(new URL("tool-operation-basis.json", import.meta.url), `${JSON.stringify(compileManagedSource(normalized), null, 2)}\n`);
} else {
  const requests = JSON.parse(readFileSync(new URL("../../../../target/m98/tool-operation-requests.json", import.meta.url), "utf8"));
  const compiled = Object.fromEntries(Object.entries(requests).map(([name, request]) => [name, applyManagedSketchMutation(request.current, request.ticket.mutation).compiled]));
  writeFileSync(new URL("tool-operation-compilations.json", import.meta.url), `${JSON.stringify(compiled, null, 2)}\n`);
}
