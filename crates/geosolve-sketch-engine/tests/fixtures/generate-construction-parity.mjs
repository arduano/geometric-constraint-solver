// SPDX-License-Identifier: GPL-3.0-or-later
// Run construction_parity::write_all_recipe_compiler_requests explicitly first.
import { readFileSync, writeFileSync } from "node:fs";
import { applyManagedSketchMutation } from "../../../../packages/geosolve-sketch-code/dist/src/managed.js";
const requests = JSON.parse(readFileSync(new URL("../../../../target/m98/construction-parity-requests.json", import.meta.url), "utf8"));
const outputs = Object.fromEntries(Object.entries(requests).map(([name, request]) => [name,
  applyManagedSketchMutation(request.current, request.ticket.mutation).compiled,
]));
writeFileSync(new URL("construction-parity-compilations.json", import.meta.url), `${JSON.stringify(outputs, null, 2)}\n`);
