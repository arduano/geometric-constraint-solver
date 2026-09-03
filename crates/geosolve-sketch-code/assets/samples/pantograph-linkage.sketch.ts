"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1PantographFixedAnchorO = $.geometry.sketchPoint("point1PantographFixedAnchorO", {
    point: [0, 0],
    label: "Pantograph fixed anchor O",
  });
  const point2PantographInputA = $.geometry.sketchPoint("point2PantographInputA", {
    point: [4, 1],
    label: "Pantograph input A",
  });
  const point3PantographGuideB = $.geometry.sketchPoint("point3PantographGuideB", {
    point: [1, 3],
    label: "Pantograph guide B",
  });
  const point4PantographOutputC = $.geometry.sketchPoint("point4PantographOutputC", {
    point: [5, 4],
    label: "Pantograph output C",
  });
  const point5PantographCenterM = $.geometry.sketchPoint("point5PantographCenterM", {
    point: [2.5, 2],
    label: "Pantograph center M",
  });
  const curve1PantographInputArmOA = $.geometry.segment("curve1PantographInputArmOA", {
    start: point1PantographFixedAnchorO.point,
    end: point2PantographInputA.point,
    branchDirection: [0.9701425001453319, 0.24253562503633297],
    label: "Pantograph input arm O-A",
    role: "profile",
  });
  const curve2PantographGuideArmOB = $.geometry.segment("curve2PantographGuideArmOB", {
    start: point1PantographFixedAnchorO.point,
    end: point3PantographGuideB.point,
    branchDirection: [0.31622776601683794, 0.9486832980505138],
    label: "Pantograph guide arm O-B",
    role: "profile",
  });
  const curve3PantographTranslatedGuideAC = $.geometry.segment("curve3PantographTranslatedGuideAC", {
    start: point2PantographInputA.point,
    end: point4PantographOutputC.point,
    branchDirection: [0.31622776601683794, 0.9486832980505138],
    label: "Pantograph translated guide A-C",
    role: "profile",
  });
  const curve4PantographTranslatedInputBC = $.geometry.segment("curve4PantographTranslatedInputBC", {
    start: point3PantographGuideB.point,
    end: point4PantographOutputC.point,
    branchDirection: [0.9701425001453319, 0.24253562503633297],
    label: "Pantograph translated input B-C",
    role: "profile",
  });
  const curve5PantographDiagonalOC = $.geometry.segment("curve5PantographDiagonalOC", {
    start: point1PantographFixedAnchorO.point,
    end: point4PantographOutputC.point,
    branchDirection: [0.7808688094430304, 0.6246950475544243],
    label: "Pantograph diagonal O-C",
    role: "profile",
  });
  const constraint1PantographAnchorFixed = $.constraint.fixedPoint("constraint1PantographAnchorFixed", {
    point: point1PantographFixedAnchorO.point,
    target: [0, 0],
    label: "Pantograph anchor fixed",
  });
  const constraint2PantographInputSidesParallel = $.constraint.parallel("constraint2PantographInputSidesParallel", {
    first: curve1PantographInputArmOA.span,
    second: curve4PantographTranslatedInputBC.span,
    label: "Pantograph input sides parallel",
  });
  const constraint3PantographGuideSidesParallel = $.constraint.parallel("constraint3PantographGuideSidesParallel", {
    first: curve2PantographGuideArmOB.span,
    second: curve3PantographTranslatedGuideAC.span,
    label: "Pantograph guide sides parallel",
  });
  const dimension1PantographInputArmLengthSqrt17 = $.dimension.curveLength("dimension1PantographInputArmLengthSqrt17", {
    curve: curve1PantographInputArmOA.span,
    value: mm(4.123105625617661),
    label: "Pantograph input arm length sqrt 17",
    mode: "driving",
  });
  const dimension2PantographGuideArmLengthSqrt10 = $.dimension.curveLength("dimension2PantographGuideArmLengthSqrt10", {
    curve: curve2PantographGuideArmOB.span,
    value: mm(3.1622776601683795),
    label: "Pantograph guide arm length sqrt 10",
    mode: "driving",
  });
  const constraint4PantographCenterBisectsDiagonal = $.constraint.midpoint("constraint4PantographCenterBisectsDiagonal", {
    point: point5PantographCenterM.point,
    line: curve5PantographDiagonalOC.span,
    label: "Pantograph center bisects diagonal",
  });
  $.group("Points", [point1PantographFixedAnchorO, point2PantographInputA, point3PantographGuideB, point4PantographOutputC, point5PantographCenterM]);
  $.group("Geometry", [curve1PantographInputArmOA, curve2PantographGuideArmOB, curve3PantographTranslatedGuideAC, curve4PantographTranslatedInputBC, curve5PantographDiagonalOC]);
  $.group("Constraints", [constraint1PantographAnchorFixed, constraint2PantographInputSidesParallel, constraint3PantographGuideSidesParallel, constraint4PantographCenterBisectsDiagonal]);
  $.group("Dimensions", [dimension1PantographInputArmLengthSqrt17, dimension2PantographGuideArmLengthSqrt10]);
  return {};
});
