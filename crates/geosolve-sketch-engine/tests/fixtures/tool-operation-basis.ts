// SPDX-License-Identifier: GPL-3.0-or-later
"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
export default sketch(($) => {
  const hline = $.geometry.segment("hline", { start: [0, 0], end: [20, 0], label: "hline" });
  const vline = $.geometry.segment("vline", { start: [0, 0], end: [0, 20], label: "vline" });
  const collinear = $.geometry.segment("collinear", { start: [30, 0], end: [50, 0], label: "collinear" });
  const parallel = $.geometry.segment("parallel", { start: [0, 10], end: [20, 10], label: "parallel" });
  const axis = $.geometry.segment("axis", { start: [20, 20], end: [20, 40], label: "axis" });
  const left = $.geometry.sketchPoint("left", { point: [10, 30], label: "left" });
  const right = $.geometry.sketchPoint("right", { point: [30, 30], label: "right" });
  const coincident = $.geometry.sketchPoint("coincident", { point: [10, 30], label: "coincident" });
  const midpoint = $.geometry.sketchPoint("midpoint", { point: [10, 0], label: "midpoint" });
  const circle = $.geometry.centerRadiusCircle("circle", { center: [100, 100], radius: mm(5), label: "circle" });
  const concentric = $.geometry.centerRadiusCircle("concentric", { center: [100, 100], radius: mm(8), label: "concentric" });
  const tangent = $.geometry.segment("tangent", { start: [90, 105], end: [110, 105], label: "tangent" });
  const incoming = $.geometry.cubicBezier("incoming", { start: [0, 100], firstControl: [10, 100], secondControl: [20, 100], end: [30, 100], label: "incoming" });
  const outgoing = $.geometry.cubicBezier("outgoing", { start: [30, 100], firstControl: [40, 100], secondControl: [50, 100], end: [60, 100], label: "outgoing" });
  const corner = $.geometry.polyline("corner", { vertices: [{ key: "start", position: [200, 0] }, { key: "corner", position: [220, 0] }, { key: "end", position: [220, 20] }], label: "corner" });
  return { hline, vline, collinear, parallel, axis, left, right, coincident, midpoint, circle, concentric, tangent, incoming, outgoing, corner };
});
