// SPDX-License-Identifier: GPL-3.0-or-later
// First run the ignored native construction::write_real_compiler_requests test.
import { readFileSync, writeFileSync } from "node:fs";
import { applyManagedSketchMutation } from "../../../../packages/geosolve-sketch-code/dist/src/managed.js";
const requests = JSON.parse(readFileSync(new URL("../../../../target/m98/construction-requests.json", import.meta.url), "utf8"));
const outputs = Object.fromEntries(Object.entries(requests).map(([name, request]) => [name,
  applyManagedSketchMutation(request.current, request.ticket.mutation).compiled,
]));
writeFileSync(new URL("construction-compilations.json", import.meta.url), `${JSON.stringify(outputs, null, 2)}\n`);
