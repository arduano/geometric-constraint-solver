"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // This planar diagram combines the published Link assembly envelope with
  // the README's qualitative three-point Maxwell-coupling architecture. Seat
  // and board coordinates are schematic, not extracted manufacturing data.
  // All seats follow the driving pitch circle as an equilateral triangle;
  // explicit contact sectors preserve the three-point assembly on edits.
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
  const upperCouplingOnPitch = $.constraint.pointOnCurve("upperCouplingOnPitch", {
    point: upperCoupling.center,
    curve: couplingPitch.span,
    contact: {
      parameter: 1.5707963267948966,
      winding: 0,
      range: {
        lower: 1,
        upper: 2,
      },
      neighborhood: {
        kind: "interior",
      },
      orientation: "none",
    },
    label: "upperCoupling retained on coupling pitch",
  });
  const lowerRightCouplingOnPitch = $.constraint.pointOnCurve("lowerRightCouplingOnPitch", {
    point: lowerRightCoupling.center,
    curve: couplingPitch.span,
    contact: {
      parameter: 5.759586531581287,
      winding: 0,
      range: {
        lower: 5,
        upper: 6.2,
      },
      neighborhood: {
        kind: "interior",
      },
      orientation: "none",
    },
    label: "lowerRightCoupling retained on coupling pitch",
  });
  const lowerLeftCouplingOnPitch = $.constraint.pointOnCurve("lowerLeftCouplingOnPitch", {
    point: lowerLeftCoupling.center,
    curve: couplingPitch.span,
    contact: {
      parameter: 3.665191429188092,
      winding: 0,
      range: {
        lower: 3.2,
        upper: 4.1,
      },
      neighborhood: {
        kind: "interior",
      },
      orientation: "none",
    },
    label: "lowerLeftCoupling retained on coupling pitch",
  });
  const couplingAxis = $.constraint.pointOnDatumAxis("couplingAxis", {
    point: upperCoupling.center,
    axis: "y",
    label: "Upper seat defines coupling orientation",
  });
  const equalCouplingLeg1 = $.constraint.equalLength("equalCouplingLeg1", {
    first: upperToLowerRight.span,
    second: lowerCouplingBase.span,
    label: "Equilateral coupling triangle first pair",
  });
  const equalCouplingLeg2 = $.constraint.equalLength("equalCouplingLeg2", {
    first: upperToLowerRight.span,
    second: lowerLeftToUpper.span,
    label: "Equilateral coupling triangle second pair",
  });
  $.group("Tool-link envelope", [toolLinkEnvelope]);
  $.group("Three-point coupling", [couplingPitch, upperCoupling, lowerRightCoupling, lowerLeftCoupling, locateCouplingPitch, couplingPitchRadius, couplingSeatRadius, matchLowerRightCoupling, matchLowerLeftCoupling, upperToLowerRight, lowerCouplingBase, lowerLeftToUpper, upperCouplingOnPitch, lowerRightCouplingOnPitch, lowerLeftCouplingOnPitch, couplingAxis, equalCouplingLeg1, equalCouplingLeg2]);
  $.group("Link-board interface", [linkBoardEnvelope, boardMountUpper, boardMountLower, locateBoardMountUpper, locateBoardMountLower, boardMountRadius, matchBoardMount]);
  return {};
});
