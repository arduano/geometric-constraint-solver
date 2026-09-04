"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const crankPivot = $.geometry.sketchPoint("crankPivot", {
    point: [0, 0],
    label: "Crank pivot",
  });
  const rockerPivot = $.geometry.sketchPoint("rockerPivot", {
    point: [5, 0],
    label: "Slotted-rocker pivot",
  });
  const crankPin = $.geometry.sketchPoint("crankPin", {
    point: [2, 2],
    label: "Sliding crank pin",
  });
  const rockerEnd = $.geometry.sketchPoint("rockerEnd", {
    point: [-4, 6],
    label: "Rocker end",
  });
  const ramPin = $.geometry.sketchPoint("ramPin", {
    point: [4, 6],
    label: "Ram pin",
  });
  const frame = $.geometry.segment("frame", {
    start: crankPivot.point,
    end: rockerPivot.point,
    branchDirection: [1, 0],
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
    branchDirection: [-0.8320502943378437, 0.5547001962252291],
    label: "Slotted rocker",
    role: "profile",
  });
  const returnLink = $.geometry.segment("returnLink", {
    start: rockerEnd.point,
    end: ramPin.point,
    branchDirection: [1, 0],
    label: "Return link",
    role: "profile",
  });
  const frameAnchor = $.constraint.fixedPoint("frameAnchor", {
    point: crankPivot.point,
    target: [0, 0],
    label: "Frame datum",
  });
  const frameAxis = $.constraint.horizontal("frameAxis", {
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
      parameter: 0.3333333333333333,
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
    value: mm(7.211102550927978),
    label: "Current ram reach",
    mode: "reference",
  });
  $.group("Ground pivots", [crankPivot, rockerPivot, frame, frameAnchor, frameAxis, frameLength]);
  $.group("Crank and slot", [crankPin, rockerEnd, crank, slottedRocker, crankLength, rockerLength, crankPinInSlot]);
  $.group("Quick-return ram", [ramPin, returnLink, returnLinkLength, ramGuide, strokeReference]);
  return {};
});
