// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { declarationSourceProjection } from "../packages/geosolve-cli/runtime/collaboration-domain-syntax.mjs";
test("compiler source projection explicitly converts Unicode AST offsets to workbench UTF8 spans", () => {
  const canonical = 'export default sketch(($)=>{const bore=$.geometry.centerRadiusCircle("bore",{center:[0,0],radius:mm(2)});return {bore};});';
  const raw = '// Unicode 😀 é\n' + canonical.replace('const bore=', '/* preceding 😀 */ const bore = ');
  const projection = declarationSourceProjection(canonical, raw, 'nested/drawing.ts'), span = projection.spans[0];
  assert.equal(projection.path, 'nested/drawing.ts');
  assert.equal(Buffer.from(raw).subarray(span.raw.from, span.raw.to).toString(), 'const bore = $.geometry.centerRadiusCircle("bore",{center:[0,0],radius:mm(2)});');
  assert.equal(Buffer.from(canonical).subarray(span.canonical.from, span.canonical.to).toString(), 'const bore=$.geometry.centerRadiusCircle("bore",{center:[0,0],radius:mm(2)});');
  assert.notEqual(span.raw.from, raw.indexOf('const bore'));
});
