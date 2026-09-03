"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1RotatingSquareAnchorA = $.geometry.sketchPoint("point1RotatingSquareAnchorA", {
    point: [0, 0],
    label: "Rotating square anchor A",
  });
  const point2RotatingSquareCornerB = $.geometry.sketchPoint("point2RotatingSquareCornerB", {
    point: [3, 0],
    label: "Rotating square corner B",
  });
  const point3RotatingSquareCornerC = $.geometry.sketchPoint("point3RotatingSquareCornerC", {
    point: [3, 3],
    label: "Rotating square corner C",
  });
  const point4RotatingSquareCornerD = $.geometry.sketchPoint("point4RotatingSquareCornerD", {
    point: [0, 3],
    label: "Rotating square corner D",
  });
  const curve1RotatingSquareEdgeAb = $.geometry.segment("curve1RotatingSquareEdgeAb", {
    start: point1RotatingSquareAnchorA.point,
    end: point2RotatingSquareCornerB.point,
    branchDirection: [1, 0],
    label: "Rotating square edge AB",
    role: "profile",
  });
  const curve2RotatingSquareEdgeBc = $.geometry.segment("curve2RotatingSquareEdgeBc", {
    start: point2RotatingSquareCornerB.point,
    end: point3RotatingSquareCornerC.point,
    branchDirection: [0, 1],
    label: "Rotating square edge BC",
    role: "profile",
  });
  const curve3RotatingSquareEdgeCd = $.geometry.segment("curve3RotatingSquareEdgeCd", {
    start: point3RotatingSquareCornerC.point,
    end: point4RotatingSquareCornerD.point,
    branchDirection: [-1, 0],
    label: "Rotating square edge CD",
    role: "profile",
  });
  const curve4RotatingSquareEdgeDa = $.geometry.segment("curve4RotatingSquareEdgeDa", {
    start: point4RotatingSquareCornerD.point,
    end: point1RotatingSquareAnchorA.point,
    branchDirection: [0, -1],
    label: "Rotating square edge DA",
    role: "profile",
  });
  const constraint1RotatingSquareAnchorFixed = $.constraint.fixedPoint("constraint1RotatingSquareAnchorFixed", {
    point: point1RotatingSquareAnchorA.point,
    target: [0, 0],
    label: "Rotating square anchor fixed",
  });
  const dimension1RotatingSquareSideLength3 = $.dimension.curveLength("dimension1RotatingSquareSideLength3", {
    curve: curve1RotatingSquareEdgeAb.span,
    value: mm(3),
    label: "Rotating square side length 3",
    mode: "driving",
  });
  const constraint2RotatingSquareAdjacentEdgesPerpendicular = $.constraint.perpendicular("constraint2RotatingSquareAdjacentEdgesPerpendicular", {
    first: curve1RotatingSquareEdgeAb.span,
    second: curve2RotatingSquareEdgeBc.span,
    label: "Rotating square adjacent edges perpendicular",
  });
  const constraint3RotatingSquareAdjacentEdgesEqual = $.constraint.equalLength("constraint3RotatingSquareAdjacentEdgesEqual", {
    first: curve1RotatingSquareEdgeAb.span,
    second: curve2RotatingSquareEdgeBc.span,
    label: "Rotating square adjacent edges equal",
  });
  const constraint4RotatingSquareOppositeEdgesAbCdParallel = $.constraint.parallel("constraint4RotatingSquareOppositeEdgesAbCdParallel", {
    first: curve1RotatingSquareEdgeAb.span,
    second: curve3RotatingSquareEdgeCd.span,
    label: "Rotating square opposite edges AB CD parallel",
  });
  const constraint5RotatingSquareOppositeEdgesBcDaParallel = $.constraint.parallel("constraint5RotatingSquareOppositeEdgesBcDaParallel", {
    first: curve2RotatingSquareEdgeBc.span,
    second: curve4RotatingSquareEdgeDa.span,
    label: "Rotating square opposite edges BC DA parallel",
  });
  $.group("Points", [point1RotatingSquareAnchorA, point2RotatingSquareCornerB, point3RotatingSquareCornerC, point4RotatingSquareCornerD]);
  $.group("Geometry", [curve1RotatingSquareEdgeAb, curve2RotatingSquareEdgeBc, curve3RotatingSquareEdgeCd, curve4RotatingSquareEdgeDa]);
  $.group("Constraints", [constraint1RotatingSquareAnchorFixed, constraint2RotatingSquareAdjacentEdgesPerpendicular, constraint3RotatingSquareAdjacentEdgesEqual, constraint4RotatingSquareOppositeEdgesAbCdParallel, constraint5RotatingSquareOppositeEdgesBcDaParallel]);
  $.group("Dimensions", [dimension1RotatingSquareSideLength3]);
  return {};
});
