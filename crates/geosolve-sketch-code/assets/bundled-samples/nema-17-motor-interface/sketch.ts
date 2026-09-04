"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // Independently drawn NEMA 17 mounting-face study. The 42.3 mm face,
  // 31 mm mounting square, pilot and shaft clearances are editable sketch
  // intent; this is an interface study rather than an OEM motor drawing.
  const motorFace = $.geometry.twoPointAlignedRectangle("motorFace", {
    firstCorner: [-21.15, -21.15],
    oppositeCorner: [21.15, 21.15],
    label: "42.3 mm motor face",
    role: "profile",
  });
  const faceAnchor = $.constraint.fixedPoint("faceAnchor", {
    point: motorFace.corners[0],
    target: [-21.15, -21.15],
    label: "Face datum",
  });
  const faceWidth = $.dimension.curveLength("faceWidth", {
    curve: motorFace.spans[0],
    value: mm(42.3),
    label: "Face width",
    mode: "driving",
  });
  const faceHeight = $.dimension.curveLength("faceHeight", {
    curve: motorFace.spans[1],
    value: mm(42.3),
    label: "Face height",
    mode: "driving",
  });
  const faceDiagonal = $.geometry.segment("faceDiagonal", {
    start: motorFace.corners[0],
    end: motorFace.corners[2],
    branchDirection: [0.7071067811865475, 0.7071067811865475],
    label: "Face-centre construction diagonal",
    role: "construction",
  });
  const faceCenter = $.geometry.sketchPoint("faceCenter", {
    point: [0, 0],
    label: "Motor axis",
    role: "construction",
  });
  const faceCenterOnDiagonal = $.constraint.midpoint("faceCenterOnDiagonal", {
    point: faceCenter.point,
    line: faceDiagonal.span,
    label: "Motor axis at face centre",
  });
  const pilot = $.geometry.centerRadiusCircle("pilot", {
    center: faceCenter.point,
    radius: mm(11),
    label: "22 mm pilot clearance",
    role: "profile",
  });
  const shaft = $.geometry.centerRadiusCircle("shaft", {
    center: faceCenter.point,
    radius: mm(2.5),
    label: "5 mm shaft clearance",
    role: "profile",
  });
  const pilotRadius = $.dimension.radius("pilotRadius", {
    curve: pilot.curve,
    value: mm(11),
    label: "Pilot radius",
    mode: "driving",
  });
  const shaftRadius = $.dimension.radius("shaftRadius", {
    curve: shaft.curve,
    value: mm(2.5),
    label: "Shaft radius",
    mode: "driving",
  });
  const mountNe = $.geometry.sketchPoint("mountNe", {
    point: [15.5, 15.5],
    label: "North-east mounting centre",
    role: "construction",
  });
  const mountNw = $.geometry.sketchPoint("mountNw", {
    point: [-15.5, 15.5],
    label: "North-west mounting centre",
    role: "construction",
  });
  const mountSe = $.geometry.sketchPoint("mountSe", {
    point: [15.5, -15.5],
    label: "South-east mounting centre",
    role: "construction",
  });
  const mountSw = $.geometry.sketchPoint("mountSw", {
    point: [-15.5, -15.5],
    label: "South-west mounting centre",
    role: "construction",
  });
  const mountNeX = $.constraint.fixedCoordinate("mountNeX", {
    point: mountNe.point,
    axis: "x",
    target: mm(15.5),
    label: "Half mounting pitch X",
  });
  const mountNeY = $.constraint.fixedCoordinate("mountNeY", {
    point: mountNe.point,
    axis: "y",
    target: mm(15.5),
    label: "Half mounting pitch Y",
  });
  const mirrorNorthMounts = $.constraint.symmetricAboutDatumAxis("mirrorNorthMounts", {
    first: mountNw.point,
    second: mountNe.point,
    axis: "y",
    label: "North mounting pair symmetry",
  });
  const mirrorEastMounts = $.constraint.symmetricAboutDatumAxis("mirrorEastMounts", {
    first: mountSe.point,
    second: mountNe.point,
    axis: "x",
    label: "East mounting pair symmetry",
  });
  const mirrorWestMounts = $.constraint.symmetricAboutDatumAxis("mirrorWestMounts", {
    first: mountSw.point,
    second: mountNw.point,
    axis: "x",
    label: "West mounting pair symmetry",
  });
  const holeNe = $.geometry.centerRadiusCircle("holeNe", {
    center: mountNe.point,
    radius: mm(1.6),
    label: "NE M3 clearance",
    role: "profile",
  });
  const holeNw = $.geometry.centerRadiusCircle("holeNw", {
    center: mountNw.point,
    radius: mm(1.6),
    label: "NW M3 clearance",
    role: "profile",
  });
  const holeSe = $.geometry.centerRadiusCircle("holeSe", {
    center: mountSe.point,
    radius: mm(1.6),
    label: "SE M3 clearance",
    role: "profile",
  });
  const holeSw = $.geometry.centerRadiusCircle("holeSw", {
    center: mountSw.point,
    radius: mm(1.6),
    label: "SW M3 clearance",
    role: "profile",
  });
  const holeNeRadius = $.dimension.radius("holeNeRadius", {
    curve: holeNe.curve,
    value: mm(1.6),
    label: "NE clearance radius",
    mode: "driving",
  });
  const holeNwRadius = $.dimension.radius("holeNwRadius", {
    curve: holeNw.curve,
    value: mm(1.6),
    label: "NW clearance radius",
    mode: "driving",
  });
  const holeSeRadius = $.dimension.radius("holeSeRadius", {
    curve: holeSe.curve,
    value: mm(1.6),
    label: "SE clearance radius",
    mode: "driving",
  });
  const holeSwRadius = $.dimension.radius("holeSwRadius", {
    curve: holeSw.curve,
    value: mm(1.6),
    label: "SW clearance radius",
    mode: "driving",
  });
  $.group("Motor face datum", [motorFace, faceAnchor, faceWidth, faceHeight, faceDiagonal, faceCenter, faceCenterOnDiagonal]);
  $.group("Pilot and shaft interface", [pilot, shaft, pilotRadius, shaftRadius]);
  $.group("31 mm mounting pattern", [mountNe, mountNw, mountSe, mountSw, mountNeX, mountNeY, mirrorNorthMounts, mirrorEastMounts, mirrorWestMounts, holeNe, holeNw, holeSe, holeSw, holeNeRadius, holeNwRadius, holeSeRadius, holeSwRadius]);
  return {};
});
