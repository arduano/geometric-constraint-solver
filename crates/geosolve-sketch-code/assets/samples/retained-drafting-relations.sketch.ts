"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1HorizontalRelationFirstPoint = $.geometry.sketchPoint("point1HorizontalRelationFirstPoint", {
    point: [-10, 6],
    label: "Horizontal relation first point",
  });
  const point2HorizontalRelationSecondPoint = $.geometry.sketchPoint("point2HorizontalRelationSecondPoint", {
    point: [-4, 6],
    label: "Horizontal relation second point",
  });
  const point3VerticalRelationFirstPoint = $.geometry.sketchPoint("point3VerticalRelationFirstPoint", {
    point: [-10, -1],
    label: "Vertical relation first point",
  });
  const point4VerticalRelationSecondPoint = $.geometry.sketchPoint("point4VerticalRelationSecondPoint", {
    point: [-10, -7],
    label: "Vertical relation second point",
  });
  const point5ConcentricRelationSharedPositionA = $.geometry.sketchPoint("point5ConcentricRelationSharedPositionA", {
    point: [2, 5],
    label: "Concentric relation shared position A",
  });
  const point6ConcentricRelationSharedPositionB = $.geometry.sketchPoint("point6ConcentricRelationSharedPositionB", {
    point: [2, 5],
    label: "Concentric relation shared position B",
  });
  const point7CollinearFirstStart = $.geometry.sketchPoint("point7CollinearFirstStart", {
    point: [7, -3],
    label: "Collinear first start",
  });
  const point8CollinearFirstEnd = $.geometry.sketchPoint("point8CollinearFirstEnd", {
    point: [11, -1],
    label: "Collinear first end",
  });
  const point9CollinearSecondStart = $.geometry.sketchPoint("point9CollinearSecondStart", {
    point: [12, -0.5],
    label: "Collinear second start",
  });
  const point10CollinearSecondEnd = $.geometry.sketchPoint("point10CollinearSecondEnd", {
    point: [16, 1.5],
    label: "Collinear second end",
  });
  const curve1ConcentricOuterCircle = $.geometry.centerRadiusCircle("curve1ConcentricOuterCircle", {
    center: point5ConcentricRelationSharedPositionA.point,
    radius: mm(3),
    label: "Concentric outer circle",
    role: "profile",
  });
  const curve2ConcentricInnerCircle = $.geometry.centerRadiusCircle("curve2ConcentricInnerCircle", {
    center: point6ConcentricRelationSharedPositionB.point,
    radius: mm(1.5),
    label: "Concentric inner circle",
    role: "profile",
  });
  const curve3CollinearRelationFirstSupport = $.geometry.segment("curve3CollinearRelationFirstSupport", {
    start: point7CollinearFirstStart.point,
    end: point8CollinearFirstEnd.point,
    branchDirection: [0.8944271909999159, 0.4472135954999579],
    label: "Collinear relation first support",
    role: "profile",
  });
  const curve4CollinearRelationSecondSupport = $.geometry.segment("curve4CollinearRelationSecondSupport", {
    start: point9CollinearSecondStart.point,
    end: point10CollinearSecondEnd.point,
    branchDirection: [0.8944271909999159, 0.4472135954999579],
    label: "Collinear relation second support",
    role: "profile",
  });
  const constraint1RetainedHorizontalPoints = $.constraint.horizontalPoints("constraint1RetainedHorizontalPoints", {
    first: point1HorizontalRelationFirstPoint.point,
    second: point2HorizontalRelationSecondPoint.point,
    label: "Retained horizontal points",
  });
  const constraint2RetainedVerticalPoints = $.constraint.verticalPoints("constraint2RetainedVerticalPoints", {
    first: point3VerticalRelationFirstPoint.point,
    second: point4VerticalRelationSecondPoint.point,
    label: "Retained vertical points",
  });
  const constraint3RetainedConcentricCurves = $.constraint.concentric("constraint3RetainedConcentricCurves", {
    first: curve1ConcentricOuterCircle.curve,
    second: curve2ConcentricInnerCircle.curve,
    label: "Retained concentric curves",
  });
  const constraint4RetainedCollinearSupports = $.constraint.collinear("constraint4RetainedCollinearSupports", {
    first: curve3CollinearRelationFirstSupport.span,
    second: curve4CollinearRelationSecondSupport.span,
    firstDirection: "forward",
    secondDirection: "forward",
    label: "Retained collinear supports",
  });
  $.group("Points", [point1HorizontalRelationFirstPoint, point2HorizontalRelationSecondPoint, point3VerticalRelationFirstPoint, point4VerticalRelationSecondPoint, point5ConcentricRelationSharedPositionA, point6ConcentricRelationSharedPositionB, point7CollinearFirstStart, point8CollinearFirstEnd, point9CollinearSecondStart, point10CollinearSecondEnd]);
  $.group("Geometry", [curve1ConcentricOuterCircle, curve2ConcentricInnerCircle, curve3CollinearRelationFirstSupport, curve4CollinearRelationSecondSupport]);
  $.group("Constraints", [constraint1RetainedHorizontalPoints, constraint2RetainedVerticalPoints, constraint3RetainedConcentricCurves, constraint4RetainedCollinearSupports]);
  return {};
});
