"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1A1RectangleBottomLeft = $.geometry.sketchPoint("point1A1RectangleBottomLeft", {
    point: [0, 0],
    label: "A1 rectangle.bottom_left",
  });
  const point2A1RectangleBottomRight = $.geometry.sketchPoint("point2A1RectangleBottomRight", {
    point: [4, 0],
    label: "A1 rectangle.bottom_right",
  });
  const point3A1RectangleTopRight = $.geometry.sketchPoint("point3A1RectangleTopRight", {
    point: [4, 3],
    label: "A1 rectangle.top_right",
  });
  const point4A1RectangleTopLeft = $.geometry.sketchPoint("point4A1RectangleTopLeft", {
    point: [0, 3],
    label: "A1 rectangle.top_left",
  });
  const point5OverlappingGuideStart = $.geometry.sketchPoint("point5OverlappingGuideStart", {
    point: [0, 0],
    label: "Overlapping guide start",
  });
  const point6OverlappingGuideEnd = $.geometry.sketchPoint("point6OverlappingGuideEnd", {
    point: [4, 0],
    label: "Overlapping guide end",
  });
  const curve1A1RectangleEdge1 = $.geometry.segment("curve1A1RectangleEdge1", {
    start: point1A1RectangleBottomLeft.point,
    end: point2A1RectangleBottomRight.point,
    branchDirection: [1, 0],
    label: "A1 rectangle.edge_1",
    role: "profile",
  });
  const curve2A1RectangleEdge2 = $.geometry.segment("curve2A1RectangleEdge2", {
    start: point2A1RectangleBottomRight.point,
    end: point3A1RectangleTopRight.point,
    branchDirection: [0, 1],
    label: "A1 rectangle.edge_2",
    role: "profile",
  });
  const curve3A1RectangleEdge3 = $.geometry.segment("curve3A1RectangleEdge3", {
    start: point3A1RectangleTopRight.point,
    end: point4A1RectangleTopLeft.point,
    branchDirection: [-1, 0],
    label: "A1 rectangle.edge_3",
    role: "profile",
  });
  const curve4A1RectangleEdge4 = $.geometry.segment("curve4A1RectangleEdge4", {
    start: point4A1RectangleTopLeft.point,
    end: point1A1RectangleBottomLeft.point,
    branchDirection: [0, -1],
    label: "A1 rectangle.edge_4",
    role: "profile",
  });
  const curve5SharedCornerConstructionDiagonal = $.geometry.segment("curve5SharedCornerConstructionDiagonal", {
    start: point1A1RectangleBottomLeft.point,
    end: point3A1RectangleTopRight.point,
    branchDirection: [0.8, 0.6],
    label: "Shared-corner construction diagonal",
    role: "construction",
  });
  const curve6ConstructionGuideOverlappingTheProfileBase = $.geometry.segment("curve6ConstructionGuideOverlappingTheProfileBase", {
    start: point5OverlappingGuideStart.point,
    end: point6OverlappingGuideEnd.point,
    branchDirection: [1, 0],
    label: "Construction guide overlapping the profile base",
    role: "construction",
  });
  const constraint1A1RectangleAnchor = $.constraint.fixedPoint("constraint1A1RectangleAnchor", {
    point: point1A1RectangleBottomLeft.point,
    target: [0, 0],
    label: "A1 rectangle.anchor",
  });
  const constraint2A1RectangleBottomHorizontal = $.constraint.horizontal("constraint2A1RectangleBottomHorizontal", {
    span: curve1A1RectangleEdge1.span,
    label: "A1 rectangle.bottom_horizontal",
  });
  const constraint3A1RectangleRightVertical = $.constraint.vertical("constraint3A1RectangleRightVertical", {
    span: curve2A1RectangleEdge2.span,
    label: "A1 rectangle.right_vertical",
  });
  const constraint4A1RectangleTopHorizontal = $.constraint.horizontal("constraint4A1RectangleTopHorizontal", {
    span: curve3A1RectangleEdge3.span,
    label: "A1 rectangle.top_horizontal",
  });
  const constraint5A1RectangleLeftVertical = $.constraint.vertical("constraint5A1RectangleLeftVertical", {
    span: curve4A1RectangleEdge4.span,
    label: "A1 rectangle.left_vertical",
  });
  const dimension1Width4 = $.dimension.curveLength("dimension1Width4", {
    curve: curve1A1RectangleEdge1.span,
    value: mm(4),
    label: "width-4",
    mode: "driving",
  });
  const dimension2A1RectangleHeightDimension = $.dimension.curveLength("dimension2A1RectangleHeightDimension", {
    curve: curve2A1RectangleEdge2.span,
    value: mm(3),
    label: "A1 rectangle.height_dimension",
    mode: "driving",
  });
  const dimension3A1DiagonalReference = $.dimension.pointDistance("dimension3A1DiagonalReference", {
    first: point1A1RectangleBottomLeft.point,
    second: point3A1RectangleTopRight.point,
    value: mm(5),
    label: "A1 diagonal reference",
    mode: "reference",
  });
  const constraint6FixOverlappingConstructionGuideControl1 = $.constraint.fixedPoint("constraint6FixOverlappingConstructionGuideControl1", {
    point: point5OverlappingGuideStart.point,
    target: [0, 0],
    label: "Fix Overlapping construction guide control 1",
  });
  const constraint7FixOverlappingConstructionGuideControl2 = $.constraint.fixedPoint("constraint7FixOverlappingConstructionGuideControl2", {
    point: point6OverlappingGuideEnd.point,
    target: [4, 0],
    label: "Fix Overlapping construction guide control 2",
  });
  $.group("Points", [point1A1RectangleBottomLeft, point2A1RectangleBottomRight, point3A1RectangleTopRight, point4A1RectangleTopLeft, point5OverlappingGuideStart, point6OverlappingGuideEnd]);
  $.group("Geometry", [curve1A1RectangleEdge1, curve2A1RectangleEdge2, curve3A1RectangleEdge3, curve4A1RectangleEdge4, curve5SharedCornerConstructionDiagonal, curve6ConstructionGuideOverlappingTheProfileBase]);
  $.group("Constraints", [constraint1A1RectangleAnchor, constraint2A1RectangleBottomHorizontal, constraint3A1RectangleRightVertical, constraint4A1RectangleTopHorizontal, constraint5A1RectangleLeftVertical, constraint6FixOverlappingConstructionGuideControl1, constraint7FixOverlappingConstructionGuideControl2]);
  $.group("Dimensions", [dimension1Width4, dimension2A1RectangleHeightDimension, dimension3A1DiagonalReference]);
  return {};
});
