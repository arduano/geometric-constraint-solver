// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const geometry1 = $.geometry.centerRadiusCircle("geometry1", {
    center: [-3.599999564034599, 1.7571432931082598],
    label: "center-radius-circle-0000000000000001",
    radius: mm(1.7999986921037952),
    role: "profile",
  });
  const geometry2 = $.geometry.centerRadiusCircle("geometry2", {
    center: [2.999999999999998, 1.7571432931082598],
    label: "center-radius-circle-0000000000000002",
    radius: mm(1.8000008719308043),
    role: "profile",
  });
  const segment3 = $.geometry.segment("segment3", {
    branchDirection: [1, -1.4802973661655742e-16],
    end: [1.1999991280705136, 1.75714111328125],
    label: "segment-0000000000000002",
    role: "profile",
    start: [-1.8000008719321237, 1.7571411132812504],
  });
  const constraint5 = $.constraint.pointOnCurve("constraint5", {
    contact: {
      domain: { kind: "periodic", period: 6.283185307179586 },
      neighborhood: { kind: "interior" },
      orientation: "none",
      parameter: 6.283184096163701,
      winding: 0,
    },
    curve: geometry1.span,
    label: "segment-0000000000000002-relation-0000",
    point: segment3.start,
  });
  const constraint6 = $.constraint.pointOnCurve("constraint6", {
    contact: {
      domain: { kind: "periodic", period: 6.283185307179586 },
      neighborhood: { kind: "interior" },
      orientation: "none",
      parameter: 3.141593864604212,
      winding: 0,
    },
    curve: geometry2.span,
    label: "segment-0000000000000002-relation-0001",
    point: segment3.end,
  });
  const constraint4 = $.constraint.horizontal("constraint4", {
    label: "segment-0000000000000002-relation-0002",
    span: segment3.span,
  });
  $.group("Canvas additions", [geometry1, geometry2, segment3, constraint5, constraint6, constraint4]);
  return {};
});
