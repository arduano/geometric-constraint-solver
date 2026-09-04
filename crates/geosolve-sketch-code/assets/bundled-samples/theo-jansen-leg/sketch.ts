"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // A compact, independently authored Jansen-style planar leg: one crank input
  // drives a seven-bar pinned graph while the foot remains an unconstrained output.
  const groundPivot = $.geometry.sketchPoint("groundPivot", {
    point: [-10, 0],
    label: "Ground pivot",
  });
  const framePivot = $.geometry.sketchPoint("framePivot", {
    point: [-4, 0],
    label: "Frame pivot",
  });
  const crankPin = $.geometry.sketchPoint("crankPin", {
    point: [-8, 3],
    label: "Crank pin",
  });
  const upperKnee = $.geometry.sketchPoint("upperKnee", {
    point: [-1, 5],
    label: "Upper knee",
  });
  const lowerKnee = $.geometry.sketchPoint("lowerKnee", {
    point: [1, 0],
    label: "Lower knee",
  });
  const foot = $.geometry.sketchPoint("foot", {
    point: [4, -5],
    label: "Foot path output",
  });

  const frame = $.geometry.segment("frame", {
    start: groundPivot.point,
    end: framePivot.point,
    branchDirection: [1, 0],
    label: "Pinned frame",
    role: "construction",
  });
  const crank = $.geometry.segment("crank", {
    start: groundPivot.point,
    end: crankPin.point,
    branchDirection: [0.5547001962252291, 0.8320502943378437],
    label: "Input crank",
    role: "profile",
  });
  const crankCoupler = $.geometry.segment("crankCoupler", {
    start: crankPin.point,
    end: upperKnee.point,
    branchDirection: [0.9615239476408232, 0.27472112789737807],
    label: "Crank coupler",
    role: "profile",
  });
  const upperRocker = $.geometry.segment("upperRocker", {
    start: framePivot.point,
    end: upperKnee.point,
    branchDirection: [0.5144957554275265, 0.8574929257125441],
    label: "Upper rocker",
    role: "profile",
  });
  const kneeLink = $.geometry.segment("kneeLink", {
    start: upperKnee.point,
    end: lowerKnee.point,
    branchDirection: [0.3713906763541037, -0.9284766908852594],
    label: "Knee link",
    role: "profile",
  });
  const lowerRocker = $.geometry.segment("lowerRocker", {
    start: framePivot.point,
    end: lowerKnee.point,
    branchDirection: [1, 0],
    label: "Lower rocker",
    role: "profile",
  });
  const lowerFootLink = $.geometry.segment("lowerFootLink", {
    start: lowerKnee.point,
    end: foot.point,
    branchDirection: [0.5144957554275265, -0.8574929257125441],
    label: "Lower foot link",
    role: "profile",
  });
  const upperFootLink = $.geometry.segment("upperFootLink", {
    start: upperKnee.point,
    end: foot.point,
    branchDirection: [0.4472135954999579, -0.8944271909999159],
    label: "Upper foot link",
    role: "profile",
  });

  const groundAnchor = $.constraint.fixedPoint("groundAnchor", {
    point: groundPivot.point,
    target: [-10, 0],
    label: "Ground datum",
  });
  const frameAxis = $.constraint.horizontal("frameAxis", {
    span: frame.span,
    label: "Horizontal frame datum",
  });
  const frameLength = $.dimension.curveLength("frameLength", {
    curve: frame.span,
    value: mm(6),
    label: "Frame pivot spacing",
    mode: "driving",
  });
  const crankLength = $.dimension.curveLength("crankLength", {
    curve: crank.span,
    value: mm(3.605551275463989),
    label: "Input crank length",
    mode: "driving",
  });
  const crankCouplerLength = $.dimension.curveLength("crankCouplerLength", {
    curve: crankCoupler.span,
    value: mm(7.280109889280518),
    label: "Crank coupler length",
    mode: "driving",
  });
  const upperRockerLength = $.dimension.curveLength("upperRockerLength", {
    curve: upperRocker.span,
    value: mm(5.830951894845301),
    label: "Upper rocker length",
    mode: "driving",
  });
  const kneeLength = $.dimension.curveLength("kneeLength", {
    curve: kneeLink.span,
    value: mm(5.385164807134504),
    label: "Knee link length",
    mode: "driving",
  });
  const lowerRockerLength = $.dimension.curveLength("lowerRockerLength", {
    curve: lowerRocker.span,
    value: mm(5),
    label: "Lower rocker length",
    mode: "driving",
  });
  const lowerFootLength = $.dimension.curveLength("lowerFootLength", {
    curve: lowerFootLink.span,
    value: mm(5.830951894845301),
    label: "Lower foot link length",
    mode: "driving",
  });
  const upperFootLength = $.dimension.curveLength("upperFootLength", {
    curve: upperFootLink.span,
    value: mm(11.180339887498949),
    label: "Upper foot link length",
    mode: "driving",
  });

  $.group("Ground frame", [groundPivot, framePivot, frame, groundAnchor, frameAxis, frameLength]);
  $.group("Crank input", [crankPin, crank, crankLength]);
  $.group("Coupled leg", [upperKnee, lowerKnee, foot, crankCoupler, upperRocker, kneeLink, lowerRocker, lowerFootLink, upperFootLink, crankCouplerLength, upperRockerLength, kneeLength, lowerRockerLength, lowerFootLength, upperFootLength]);
  return {};
});
