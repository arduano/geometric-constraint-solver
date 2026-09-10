// SPDX-License-Identifier: GPL-3.0-or-later
import { writeFileSync } from 'node:fs';
const { compileManagedSource } = await import(process.env.GEOSOLVE_POINT_COMPILER_MODULE ?? new URL('../../../../packages/geosolve-sketch-code/dist/src/managed.js', import.meta.url).href);
const sources = {
  computed: `"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
export default sketch(($) => {
  const path = $.geometry.polyline("path", { vertices: [
    { key: "start", position: [0, 0] }, { key: "bend", position: [20, 0] }, { key: "end", position: [20, 20] }
  ] });
  const rounded = $.computed.filletSet("rounded", { radius: mm(2), corners: [{ key: "bend", parents: [
    { span: path.segments.byKey.start, parameter: 0.9, winding: 0, neighborhood: { kind: "interior" }, normalSide: "left", trimEndpoint: "end", periodicAnchor: { kind: "none" } },
    { span: path.segments.byKey.bend, parameter: 0.1, winding: 0, neighborhood: { kind: "interior" }, normalSide: "left", trimEndpoint: "start", periodicAnchor: { kind: "none" } }
  ], endpointOrder: "firstThenSecond", sweep: "counterClockwise" }] });
  return { path, rounded };
});`,
  constrained: `"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
export default sketch(($) => {
  const bar = $.geometry.segment("bar", { start: [0, 0], end: [20, 0] });
  const horizontal = $.constraint.horizontal("horizontal", { span: bar.span });
  const length = $.dimension.pointDistance("length", { first: bar.start, second: bar.end, value: mm(20) });
  return { bar, horizontal, length };
});`,
  shared: `"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
export default sketch(($) => {
  const producer = $.geometry.centerRadiusCircle("producer", { center: [0, 0], radius: mm(2) });
  const consumer = $.geometry.centerRadiusCircle("consumer", { center: producer.center, radius: mm(5) });
  return { producer, consumer };
});`,
  rectangle: `"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";
export default sketch(($) => {
  const box = $.geometry.twoPointAlignedRectangle("box", { firstCorner: [0, 0], oppositeCorner: [20, 10] });
  return { box };
});`,
};
for (const [name, source] of Object.entries(sources)) {
  const compiled = compileManagedSource(compileManagedSource(source).normalizedSource);
  writeFileSync(new URL(`point-gesture-${name}.json`, import.meta.url), `${JSON.stringify(compiled, null, 2)}\n`);
}
