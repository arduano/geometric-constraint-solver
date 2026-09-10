// SPDX-License-Identifier: GPL-3.0-or-later
// Run after building the managed compiler; use normalized source as actual compiler input.
import { readFileSync, writeFileSync } from 'node:fs';
import { compileManagedSource } from '../../../../packages/geosolve-sketch-code/dist/src/managed.js';
for (const radius of [2, 5]) {
  const basis = JSON.parse(readFileSync(new URL(`session-radius-${radius}.json`, import.meta.url), 'utf8'));
  const compiled = compileManagedSource(basis.normalizedSource);
  writeFileSync(new URL(`authoring-radius-${radius}.json`, import.meta.url), `${JSON.stringify(compiled, null, 2)}\n`);
}
