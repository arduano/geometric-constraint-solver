// SPDX-License-Identifier: GPL-3.0-or-later
"use geosolve sketch";
import { mm, sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  // The seed is 10; the accepted driving radius is 12. Bake must use the latter.
  const disk = $.geometry.centerRadiusCircle("disk", {
    center: [0, 0], radius: mm(10),
  });
  const diskRadius = $.dimension.radius("diskRadius", {
    curve: disk.curve, value: mm(12),
  });
  const chord = $.geometry.segment("chord", { start: [30, 5], end: [35, 0] });
  const arc = $.geometry.centerArc("arc", {
    center: [30, 0], start: chord.end, end: chord.start, sweep: "counterClockwise",
  });
  // Explicit owned endpoint contacts establish production closure (coordinates alone do not).
  const startJoin = $.constraint.pointOnCurve("startJoin", {
    point: chord.end, curve: arc.span,
    contact: { parameter: 0, winding: 0, neighborhood: { kind: "start" }, orientation: "none" },
  });
  const endJoin = $.constraint.pointOnCurve("endJoin", {
    point: chord.start, curve: arc.span,
    contact: { parameter: 1, winding: 0, neighborhood: { kind: "end" }, orientation: "none" },
  });
  return { disk, diskRadius, arc, chord, startJoin, endJoin };
});
