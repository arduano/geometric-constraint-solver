"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1TrammelHorizontalRailStart = $.geometry.sketchPoint("point1TrammelHorizontalRailStart", {
    point: [-6, 0],
    label: "Trammel horizontal rail start",
  });
  const point2TrammelHorizontalRailEnd = $.geometry.sketchPoint("point2TrammelHorizontalRailEnd", {
    point: [6, 0],
    label: "Trammel horizontal rail end",
  });
  const point3TrammelVerticalRailStart = $.geometry.sketchPoint("point3TrammelVerticalRailStart", {
    point: [0, -6],
    label: "Trammel vertical rail start",
  });
  const point4TrammelVerticalRailEnd = $.geometry.sketchPoint("point4TrammelVerticalRailEnd", {
    point: [0, 6],
    label: "Trammel vertical rail end",
  });
  const point5TrammelHorizontalSliderA = $.geometry.sketchPoint("point5TrammelHorizontalSliderA", {
    point: [4, 0],
    label: "Trammel horizontal slider A",
  });
  const point6TrammelVerticalSliderB = $.geometry.sketchPoint("point6TrammelVerticalSliderB", {
    point: [0, 3],
    label: "Trammel vertical slider B",
  });
  const point7TrammelBarMidpointM = $.geometry.sketchPoint("point7TrammelBarMidpointM", {
    point: [2, 1.5],
    label: "Trammel bar midpoint M",
  });
  const point8TrammelEllipticTracerT = $.geometry.sketchPoint("point8TrammelEllipticTracerT", {
    point: [3, 0.75],
    label: "Trammel elliptic tracer T",
  });
  const curve1TrammelHorizontalRail = $.geometry.segment("curve1TrammelHorizontalRail", {
    start: point1TrammelHorizontalRailStart.point,
    end: point2TrammelHorizontalRailEnd.point,
    branchDirection: [1, 0],
    label: "Trammel horizontal rail",
    role: "profile",
  });
  const curve2TrammelVerticalRail = $.geometry.segment("curve2TrammelVerticalRail", {
    start: point3TrammelVerticalRailStart.point,
    end: point4TrammelVerticalRailEnd.point,
    branchDirection: [0, 1],
    label: "Trammel vertical rail",
    role: "profile",
  });
  const curve3TrammelFixedLengthBarAb = $.geometry.segment("curve3TrammelFixedLengthBarAb", {
    start: point5TrammelHorizontalSliderA.point,
    end: point6TrammelVerticalSliderB.point,
    branchDirection: [-0.8, 0.6],
    label: "Trammel fixed-length bar AB",
    role: "profile",
  });
  const curve4TrammelQuarterArmAm = $.geometry.segment("curve4TrammelQuarterArmAm", {
    start: point5TrammelHorizontalSliderA.point,
    end: point7TrammelBarMidpointM.point,
    branchDirection: [-0.8, 0.6],
    label: "Trammel quarter arm AM",
    role: "profile",
  });
  const constraint1TrammelHorizontalRailStartFixed = $.constraint.fixedPoint("constraint1TrammelHorizontalRailStartFixed", {
    point: point1TrammelHorizontalRailStart.point,
    target: [-6, 0],
    label: "Trammel horizontal rail start fixed",
  });
  const constraint2TrammelHorizontalRailEndFixed = $.constraint.fixedPoint("constraint2TrammelHorizontalRailEndFixed", {
    point: point2TrammelHorizontalRailEnd.point,
    target: [6, 0],
    label: "Trammel horizontal rail end fixed",
  });
  const constraint3TrammelVerticalRailStartFixed = $.constraint.fixedPoint("constraint3TrammelVerticalRailStartFixed", {
    point: point3TrammelVerticalRailStart.point,
    target: [0, -6],
    label: "Trammel vertical rail start fixed",
  });
  const constraint4TrammelVerticalRailEndFixed = $.constraint.fixedPoint("constraint4TrammelVerticalRailEndFixed", {
    point: point4TrammelVerticalRailEnd.point,
    target: [0, 6],
    label: "Trammel vertical rail end fixed",
  });
  const constraint5TrammelASlidesHorizontally = $.constraint.pointOnCurve("constraint5TrammelASlidesHorizontally", {
    point: point5TrammelHorizontalSliderA.point,
    curve: curve1TrammelHorizontalRail.span,
    contact: {
      parameter: 0.8333333333333334,
      winding: 0,
      neighborhood: {
        kind: "interior",
      },
      orientation: "none",
    },
    label: "Trammel A slides horizontally",
  });
  const constraint6TrammelBSlidesVertically = $.constraint.pointOnCurve("constraint6TrammelBSlidesVertically", {
    point: point6TrammelVerticalSliderB.point,
    curve: curve2TrammelVerticalRail.span,
    contact: {
      parameter: 0.75,
      winding: 0,
      neighborhood: {
        kind: "interior",
      },
      orientation: "none",
    },
    label: "Trammel B slides vertically",
  });
  const dimension1TrammelBarLength5 = $.dimension.curveLength("dimension1TrammelBarLength5", {
    curve: curve3TrammelFixedLengthBarAb.span,
    value: mm(5),
    label: "Trammel bar length 5",
    mode: "driving",
  });
  const constraint7TrammelMBisectsAb = $.constraint.midpoint("constraint7TrammelMBisectsAb", {
    point: point7TrammelBarMidpointM.point,
    line: curve3TrammelFixedLengthBarAb.span,
    label: "Trammel M bisects AB",
  });
  const constraint8TrammelTBisectsAm = $.constraint.midpoint("constraint8TrammelTBisectsAm", {
    point: point8TrammelEllipticTracerT.point,
    line: curve4TrammelQuarterArmAm.span,
    label: "Trammel T bisects AM",
  });
  $.group("Points", [point1TrammelHorizontalRailStart, point2TrammelHorizontalRailEnd, point3TrammelVerticalRailStart, point4TrammelVerticalRailEnd, point5TrammelHorizontalSliderA, point6TrammelVerticalSliderB, point7TrammelBarMidpointM, point8TrammelEllipticTracerT]);
  $.group("Geometry", [curve1TrammelHorizontalRail, curve2TrammelVerticalRail, curve3TrammelFixedLengthBarAb, curve4TrammelQuarterArmAm]);
  $.group("Constraints", [constraint1TrammelHorizontalRailStartFixed, constraint2TrammelHorizontalRailEndFixed, constraint3TrammelVerticalRailStartFixed, constraint4TrammelVerticalRailEndFixed, constraint5TrammelASlidesHorizontally, constraint6TrammelBSlidesVertically, constraint7TrammelMBisectsAb, constraint8TrammelTBisectsAm]);
  $.group("Dimensions", [dimension1TrammelBarLength5]);
  return {};
});
