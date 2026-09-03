// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { deg, mm, sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const splitSource = $.geometry.segment("splitSource", { start: [0, 0], end: [8, 0] });
  const split = $.operation.split("split", {
    source: splitSource.span,
    parameter: 0.5,
    retained: "before",
  });

  const breakSource = $.geometry.segment("breakSource", { start: [0, 10], end: [8, 10] });
  const breakCurve = $.operation.break("breakCurve", {
    source: breakSource.span,
    start: 0.25,
    end: 0.75,
    retained: "before",
  });

  const trimSource = $.geometry.segment("trimSource", { start: [0, 20], end: [8, 20] });
  const trim = $.operation.trim("trim", {
    source: trimSource.span,
    parameter: 0.5,
    retained: "after",
  });

  const extendSource = $.geometry.segment("extendSource", { start: [0, 30], end: [2, 30] });
  const extendTarget = $.geometry.segment("extendTarget", { start: [4, 28], end: [4, 32] });
  const extend = $.operation.extend("extend", {
    source: extendSource.span,
    target: extendTarget.span,
    endpoint: "end",
  });

  const mirrorSource = $.geometry.quadraticBezier("mirrorSource", {
    start: [2, 40],
    control: [4, 43],
    end: [6, 40],
  });
  const mirrorAxis = $.geometry.segment("mirrorAxis", { start: [0, 36], end: [0, 44] });
  const mirror = $.operation.mirror("mirror", {
    source: mirrorSource.curve,
    axis: mirrorAxis.span,
  });

  const chamferFirst = $.geometry.segment("chamferFirst", { start: [0, 50], end: [8, 50] });
  const chamferSecond = $.geometry.segment("chamferSecond", {
    start: chamferFirst.start,
    end: [0, 58],
  });
  const chamfer = $.operation.chamfer("chamfer", {
    first: chamferFirst.span,
    second: chamferSecond.span,
    firstDistance: mm(1),
    secondDistance: mm(1),
  });

  const filletFirst = $.geometry.segment("filletFirst", { start: [0, 60], end: [8, 60] });
  const filletSecond = $.geometry.segment("filletSecond", { start: [8, 60], end: [8, 68] });
  const associativeFillet = $.operation.associativeFillet("associativeFillet", {
    radius: mm(1),
    radiusMode: "driving",
    parents: [{
      span: filletFirst.span,
      parameter: 0.75,
      winding: 0,
      neighborhood: { kind: "interior" },
      normalSide: "left",
      trimEndpoint: "end",
      periodicAnchor: { kind: "none" },
    }, {
      span: filletSecond.span,
      parameter: 0.25,
      winding: 0,
      neighborhood: { kind: "interior" },
      normalSide: "left",
      trimEndpoint: "start",
      periodicAnchor: { kind: "none" },
    }],
    endpointOrder: "firstThenSecond",
    sweep: "counterClockwise",
  });

  const rectangle = $.operation.rectangle("rectangle", {
    origin: [20, 0],
    width: mm(8),
    height: mm(6),
    role: "profile",
  });
  const regularPolygon = $.operation.regularPolygon("regularPolygon", {
    center: [24, 18],
    radius: mm(4),
    sides: 5,
    rotation: deg(18),
    role: "profile",
  });
  const slot = $.operation.slot("slot", {
    firstCenter: [20, 30],
    secondCenter: [28, 30],
    radius: mm(2),
    role: "profile",
  });

  const patternSource = $.geometry.segment("patternSource", { start: [20, 42], end: [24, 42] });
  const linearPattern = $.operation.linearPattern("linearPattern", {
    sources: [patternSource.curve],
    instances: 3,
    step: [0, 4],
  });

  const offsetSource = $.geometry.segment("offsetSource", { start: [20, 60], end: [28, 60] });
  const openChain = $.aggregate.openChain("openChain", { spans: [offsetSource.span] });
  const profileOffset = $.operation.profileOffset("profileOffset", {
    sources: [openChain.chain],
    distance: mm(1),
    side: "left",
    firstTraversal: "forward",
  });
  const closedProfile = $.aggregate.closedProfile("closedProfile", {
    spans: [
      rectangle.spans.bottom,
      rectangle.spans.right,
      rectangle.spans.top,
      rectangle.spans.left,
    ],
  });

  return {
    split,
    breakCurve,
    trim,
    extend,
    mirror,
    chamfer,
    associativeFillet,
    rectangle,
    regularPolygon,
    slot,
    linearPattern,
    openChain,
    profileOffset,
    closedProfile,
  };
});
