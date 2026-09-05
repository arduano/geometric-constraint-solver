"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // The rocker pivot is below the crank so its end swings left/right beneath
  // the horizontal ram guide. The 8 mm return link reaches every crank angle.
  // A vertical fixed frame retains the selected ground orientation explicitly.
  const crankPivot = $.geometry.sketchPoint("crankPivot", {
    point: [0, 0],
    label: "Crank pivot",
  });
  const rockerPivot = $.geometry.sketchPoint("rockerPivot", {
    point: [0, -5],
    label: "Slotted-rocker pivot",
  });
  const crankPin = $.geometry.sketchPoint("crankPin", {
    point: [2, 2],
    label: "Sliding crank pin",
  });
  const rockerEnd = $.geometry.sketchPoint("rockerEnd", {
    point: [2.971563339261892, 5.400471687416621],
    label: "Rocker end",
  });
  const ramPin = $.geometry.sketchPoint("ramPin", {
    point: [10.949067071784423, 6],
    label: "Ram pin",
  });
  const frame = $.geometry.segment("frame", {
    start: crankPivot.point,
    end: rockerPivot.point,
    branchDirection: [0, -1],
    label: "Machine frame",
    role: "construction",
  });
  const crank = $.geometry.segment("crank", {
    start: crankPivot.point,
    end: crankPin.point,
    branchDirection: [0.7071067811865475, 0.7071067811865475],
    label: "Driving crank",
    role: "profile",
  });
  const slottedRocker = $.geometry.segment("slottedRocker", {
    start: rockerPivot.point,
    end: rockerEnd.point,
    branchDirection: [0.27472112789737807, 0.9615239476408232],
    label: "Slotted rocker",
    role: "profile",
  });
  const returnLink = $.geometry.segment("returnLink", {
    start: rockerEnd.point,
    end: ramPin.point,
    branchDirection: [0.9971879665653164, 0.07494103907292238],
    label: "Return link",
    role: "profile",
  });
  const frameAnchor = $.constraint.fixedPoint("frameAnchor", {
    point: crankPivot.point,
    target: [0, 0],
    label: "Frame datum",
  });
  const frameAxis = $.constraint.vertical("frameAxis", {
    span: frame.span,
    label: "Pivot axis",
  });
  const frameLength = $.dimension.curveLength("frameLength", {
    curve: frame.span,
    value: mm(5),
    label: "Pivot spacing",
    mode: "driving",
  });
  const crankLength = $.dimension.curveLength("crankLength", {
    curve: crank.span,
    value: mm(2.8284271247461903),
    label: "Crank radius",
    mode: "driving",
  });
  const rockerLength = $.dimension.curveLength("rockerLength", {
    curve: slottedRocker.span,
    value: mm(10.816653826391969),
    label: "Slotted rocker length",
    mode: "driving",
  });
  const crankPinInSlot = $.constraint.pointOnCurve("crankPinInSlot", {
    point: crankPin.point,
    curve: slottedRocker.span,
    contact: {
      parameter: 0.673046397354189,
      winding: 0,
      neighborhood: {
        kind: "interior",
      },
      orientation: "none",
    },
    label: "Crank pin slides in rocker slot",
  });
  const returnLinkLength = $.dimension.curveLength("returnLinkLength", {
    curve: returnLink.span,
    value: mm(8),
    label: "Return-link length",
    mode: "driving",
  });
  const ramGuide = $.constraint.fixedCoordinate("ramGuide", {
    point: ramPin.point,
    axis: "y",
    target: mm(6),
    label: "Horizontal ram guide",
  });
  const strokeReference = $.dimension.pointDistance("strokeReference", {
    first: crankPivot.point,
    second: ramPin.point,
    value: mm(12.485274115630538),
    label: "Current ram reach",
    mode: "reference",
  });
  $.group("Ground pivots", [crankPivot, rockerPivot, frame, frameAnchor, frameAxis, frameLength]);
  $.group("Crank and slot", [crankPin, rockerEnd, crank, slottedRocker, crankLength, rockerLength, crankPinInSlot]);
  $.group("Quick-return ram", [ramPin, returnLink, returnLinkLength, ramGuide, strokeReference]);
  return {};
});
