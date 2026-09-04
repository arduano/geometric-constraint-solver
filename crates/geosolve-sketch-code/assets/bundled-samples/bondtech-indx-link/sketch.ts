"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // This planar diagram combines the published Link assembly envelope with
  // the README's qualitative three-point Maxwell-coupling architecture. Seat
  // and board coordinates are schematic, not extracted manufacturing data.
  const toolLinkEnvelope = $.operation.rectangle("toolLinkEnvelope", {
    origin: [-37.858, -16.773],
    width: mm(75.716),
    height: mm(33.546),
    label: "INDX Link published plan envelope",
    role: "construction",
  });
  const linkBoardEnvelope = $.operation.rectangle("linkBoardEnvelope", {
    origin: [42, -14],
    width: mm(24),
    height: mm(28),
    label: "Schematic link-board service zone",
    role: "construction",
  });
  const couplingDatum = $.operation.rectangle("couplingDatum", {
    origin: [-12.12435565298214, -7],
    width: mm(24.24871130596428),
    height: mm(21),
    label: "Schematic coupling coordinate datum",
    role: "construction",
  });
  const couplingPitch = $.geometry.centerRadiusCircle("couplingPitch", {
    center: [0, 0],
    radius: mm(14),
    label: "Schematic Maxwell coupling pitch reference",
    role: "construction",
  });
  const upperCoupling = $.geometry.centerRadiusCircle("upperCoupling", {
    center: [0, 14],
    radius: mm(2.5),
    label: "Schematic upper coupling seat",
    role: "profile",
  });
  const lowerRightCoupling = $.geometry.centerRadiusCircle("lowerRightCoupling", {
    center: [12.12435565298214, -7],
    radius: mm(2.5),
    label: "Schematic lower-right coupling seat",
    role: "profile",
  });
  const lowerLeftCoupling = $.geometry.centerRadiusCircle("lowerLeftCoupling", {
    center: [-12.12435565298214, -7],
    radius: mm(2.5),
    label: "Schematic lower-left coupling seat",
    role: "profile",
  });
  const locateCouplingPitch = $.constraint.fixedPoint("locateCouplingPitch", {
    point: couplingPitch.center,
    target: [0, 0],
    label: "Locate coupling pitch reference",
  });
  const locateUpperCoupling = $.constraint.fixedPoint("locateUpperCoupling", {
    point: upperCoupling.center,
    target: [0, 14],
    label: "Locate upper coupling seat",
  });
  const locateLowerRightCoupling = $.constraint.coincident("locateLowerRightCoupling", {
    first: lowerRightCoupling.center,
    second: couplingDatum.corners.bottomRight,
    label: "Locate lower-right coupling seat",
  });
  const locateLowerLeftCoupling = $.constraint.coincident("locateLowerLeftCoupling", {
    first: lowerLeftCoupling.center,
    second: couplingDatum.corners.bottomLeft,
    label: "Locate lower-left coupling seat",
  });
  const couplingPitchRadius = $.dimension.radius("couplingPitchRadius", {
    curve: couplingPitch.curve,
    value: mm(14),
    label: "Schematic coupling pitch radius",
    mode: "driving",
  });
  const couplingSeatRadius = $.dimension.radius("couplingSeatRadius", {
    curve: upperCoupling.curve,
    value: mm(2.5),
    label: "Schematic coupling-seat radius",
    mode: "driving",
  });
  const matchLowerRightCoupling = $.constraint.equalRadius("matchLowerRightCoupling", {
    first: upperCoupling.curve,
    second: lowerRightCoupling.curve,
    label: "Match lower-right coupling seat",
  });
  const matchLowerLeftCoupling = $.constraint.equalRadius("matchLowerLeftCoupling", {
    first: upperCoupling.curve,
    second: lowerLeftCoupling.curve,
    label: "Match lower-left coupling seat",
  });
  const upperToLowerRight = $.geometry.segment("upperToLowerRight", {
    start: upperCoupling.center,
    end: lowerRightCoupling.center,
    branchDirection: [0.5, -0.8660254037844386],
    label: "Schematic upper-to-lower-right coupling leg",
    role: "construction",
  });
  const lowerCouplingBase = $.geometry.segment("lowerCouplingBase", {
    start: lowerRightCoupling.center,
    end: lowerLeftCoupling.center,
    branchDirection: [-1, 0],
    label: "Schematic lower coupling leg",
    role: "construction",
  });
  const lowerLeftToUpper = $.geometry.segment("lowerLeftToUpper", {
    start: lowerLeftCoupling.center,
    end: upperCoupling.center,
    branchDirection: [0.5, 0.8660254037844386],
    label: "Schematic lower-left-to-upper coupling leg",
    role: "construction",
  });
  const couplingToBoard = $.operation.rectangle("couplingToBoard", {
    origin: [18, -0.25],
    width: mm(24),
    height: mm(0.5),
    label: "Schematic tool-link to board datum",
    role: "construction",
  });
  const boardMountUpper = $.geometry.centerRadiusCircle("boardMountUpper", {
    center: [66, 14],
    radius: mm(1.6),
    label: "Schematic upper link-board mount",
    role: "profile",
  });
  const boardMountLower = $.geometry.centerRadiusCircle("boardMountLower", {
    center: [66, -14],
    radius: mm(1.6),
    label: "Schematic lower link-board mount",
    role: "profile",
  });
  const locateBoardMountUpper = $.constraint.coincident("locateBoardMountUpper", {
    first: boardMountUpper.center,
    second: linkBoardEnvelope.corners.topRight,
    label: "Locate upper board mount",
  });
  const locateBoardMountLower = $.constraint.coincident("locateBoardMountLower", {
    first: boardMountLower.center,
    second: linkBoardEnvelope.corners.bottomRight,
    label: "Locate lower board mount",
  });
  const boardMountRadius = $.dimension.radius("boardMountRadius", {
    curve: boardMountUpper.curve,
    value: mm(1.6),
    label: "Schematic link-board mount radius",
    mode: "driving",
  });
  const matchBoardMount = $.constraint.equalRadius("matchBoardMount", {
    first: boardMountUpper.curve,
    second: boardMountLower.curve,
    label: "Matched link-board mounts",
  });
  $.group("Tool-link envelope", [toolLinkEnvelope, couplingToBoard]);
  $.group("Three-point coupling", [couplingDatum, couplingPitch, upperCoupling, lowerRightCoupling, lowerLeftCoupling, locateCouplingPitch, locateUpperCoupling, locateLowerRightCoupling, locateLowerLeftCoupling, couplingPitchRadius, couplingSeatRadius, matchLowerRightCoupling, matchLowerLeftCoupling, upperToLowerRight, lowerCouplingBase, lowerLeftToUpper]);
  $.group("Link-board interface", [linkBoardEnvelope, boardMountUpper, boardMountLower, locateBoardMountUpper, locateBoardMountLower, boardMountRadius, matchBoardMount]);
  return {};
});
