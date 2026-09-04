"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1PointDistanceFirstPoint = $.geometry.sketchPoint("point1PointDistanceFirstPoint", {
    point: [-20, 7],
    label: "Point-distance first point",
  });
  const point2PointDistanceSecondPoint = $.geometry.sketchPoint("point2PointDistanceSecondPoint", {
    point: [-15, 7],
    label: "Point-distance second point",
  });
  const point3StraightLengthStart = $.geometry.sketchPoint("point3StraightLengthStart", {
    point: [-11, 7],
    label: "Straight length start",
  });
  const point4StraightLengthEnd = $.geometry.sketchPoint("point4StraightLengthEnd", {
    point: [-6, 7],
    label: "Straight length end",
  });
  const point5ArcRadiusCenter = $.geometry.sketchPoint("point5ArcRadiusCenter", {
    point: [11, 7],
    label: "Arc radius center",
  });
  const point6DiameterCircleCenter = $.geometry.sketchPoint("point6DiameterCircleCenter", {
    point: [-18, -6],
    label: "Diameter circle center",
  });
  const point7AngleFirstVertex = $.geometry.sketchPoint("point7AngleFirstVertex", {
    point: [-10, -8],
    label: "Angle first vertex",
  });
  const point8AngleFirstEndpoint = $.geometry.sketchPoint("point8AngleFirstEndpoint", {
    point: [-6, -8],
    label: "Angle first endpoint",
  });
  const point9AngleSecondVertex = $.geometry.sketchPoint("point9AngleSecondVertex", {
    point: [-10, -8],
    label: "Angle second vertex",
  });
  const point10AngleSecondEndpoint = $.geometry.sketchPoint("point10AngleSecondEndpoint", {
    point: [-7, -5],
    label: "Angle second endpoint",
  });
  const point11SupportingOffsetSourceStart = $.geometry.sketchPoint("point11SupportingOffsetSourceStart", {
    point: [-1, -9],
    label: "Supporting-offset source start",
  });
  const point12SupportingOffsetSourceEnd = $.geometry.sketchPoint("point12SupportingOffsetSourceEnd", {
    point: [3, -9],
    label: "Supporting-offset source end",
  });
  const point13SupportingOffsetTargetStart = $.geometry.sketchPoint("point13SupportingOffsetTargetStart", {
    point: [0, -6],
    label: "Supporting-offset target start",
  });
  const point14SupportingOffsetTargetEnd = $.geometry.sketchPoint("point14SupportingOffsetTargetEnd", {
    point: [4, -6],
    label: "Supporting-offset target end",
  });
  const point15ExactOffsetSourceStart = $.geometry.sketchPoint("point15ExactOffsetSourceStart", {
    point: [10, -9],
    label: "Exact-offset source start",
  });
  const point16ExactOffsetSourceEnd = $.geometry.sketchPoint("point16ExactOffsetSourceEnd", {
    point: [14, -9],
    label: "Exact-offset source end",
  });
  const point17ExactOffsetTargetStart = $.geometry.sketchPoint("point17ExactOffsetTargetStart", {
    point: [10, -6],
    label: "Exact-offset target start",
  });
  const point18ExactOffsetTargetEnd = $.geometry.sketchPoint("point18ExactOffsetTargetEnd", {
    point: [14, -6],
    label: "Exact-offset target end",
  });
  const curve1StraightLengthSpecimen = $.geometry.segment("curve1StraightLengthSpecimen", {
    start: point3StraightLengthStart.point,
    end: point4StraightLengthEnd.point,
    branchDirection: [1, 0],
    label: "Straight length specimen",
    role: "profile",
  });
  const curve2CircularArcRadiusSpecimen = $.geometry.centerArc("curve2CircularArcRadiusSpecimen", {
    center: point5ArcRadiusCenter.point,
    start: [12.414213562373096, 5.585786437626905],
    end: [9.585786437626904, 8.414213562373096],
    sweep: "counterClockwise",
    label: "Circular-arc radius specimen",
    role: "profile",
  });
  const curve3FullCircleDiameterSpecimen = $.geometry.centerRadiusCircle("curve3FullCircleDiameterSpecimen", {
    center: point6DiameterCircleCenter.point,
    radius: mm(2),
    label: "Full-circle diameter specimen",
    role: "profile",
  });
  const curve4AngleFirstLeg = $.geometry.segment("curve4AngleFirstLeg", {
    start: point7AngleFirstVertex.point,
    end: point8AngleFirstEndpoint.point,
    branchDirection: [1, 0],
    label: "Angle first leg",
    role: "profile",
  });
  const curve5AngleSecondLeg = $.geometry.segment("curve5AngleSecondLeg", {
    start: point9AngleSecondVertex.point,
    end: point10AngleSecondEndpoint.point,
    branchDirection: [0.7071067811865476, 0.7071067811865476],
    label: "Angle second leg",
    role: "profile",
  });
  const curve6SupportingOffsetSource = $.geometry.segment("curve6SupportingOffsetSource", {
    start: point11SupportingOffsetSourceStart.point,
    end: point12SupportingOffsetSourceEnd.point,
    branchDirection: [1, 0],
    label: "Supporting-offset source",
    role: "profile",
  });
  const curve7SupportingOffsetTarget = $.geometry.segment("curve7SupportingOffsetTarget", {
    start: point13SupportingOffsetTargetStart.point,
    end: point14SupportingOffsetTargetEnd.point,
    branchDirection: [1, 0],
    label: "Supporting-offset target",
    role: "profile",
  });
  const curve8ExactOffsetSource = $.geometry.segment("curve8ExactOffsetSource", {
    start: point15ExactOffsetSourceStart.point,
    end: point16ExactOffsetSourceEnd.point,
    branchDirection: [1, 0],
    label: "Exact-offset source",
    role: "profile",
  });
  const curve9ExactOffsetTarget = $.geometry.segment("curve9ExactOffsetTarget", {
    start: point17ExactOffsetTargetStart.point,
    end: point18ExactOffsetTargetEnd.point,
    branchDirection: [1, 0],
    label: "Exact-offset target",
    role: "profile",
  });
  const dimension1PointDistance = $.dimension.pointDistance("dimension1PointDistance", {
    first: point1PointDistanceFirstPoint.point,
    second: point2PointDistanceSecondPoint.point,
    value: mm(5),
    label: "Point distance",
    mode: "reference",
  });
  const dimension2StraightCurveLength = $.dimension.curveLength("dimension2StraightCurveLength", {
    curve: curve1StraightLengthSpecimen.span,
    value: mm(5),
    label: "Straight curve length",
    mode: "driving",
  });
  const dimension3ArcRadius = $.dimension.radius("dimension3ArcRadius", {
    curve: curve2CircularArcRadiusSpecimen.curve,
    value: mm(2),
    label: "Arc radius",
    mode: "driving",
  });
  const dimension4CircleDiameter = $.dimension.diameter("dimension4CircleDiameter", {
    curve: curve3FullCircleDiameterSpecimen.curve,
    value: mm(4),
    label: "Circle diameter",
    mode: "driving",
  });
  const dimension5OrientedAngle = $.dimension.orientedAngle("dimension5OrientedAngle", {
    first: curve4AngleFirstLeg.span,
    second: curve5AngleSecondLeg.span,
    value: rad(0.7853981633974483),
    orientation: "counterClockwise",
    label: "Oriented angle",
    mode: "driving",
  });
  const dimension6SupportingLineOffset = $.dimension.supportingLineOffset("dimension6SupportingLineOffset", {
    first: curve6SupportingOffsetSource.span,
    second: curve7SupportingOffsetTarget.span,
    value: mm(3),
    side: "left",
    orientation: "same",
    label: "Supporting-line offset",
    mode: "driving",
  });
  const dimension7ExactTranslatedSegmentOffset = $.dimension.exactTranslatedSegmentOffset("dimension7ExactTranslatedSegmentOffset", {
    first: curve8ExactOffsetSource.span,
    second: curve9ExactOffsetTarget.span,
    value: mm(3),
    side: "left",
    orientation: "same",
    label: "Exact translated-segment offset",
    mode: "driving",
  });
  $.group("Points", [point1PointDistanceFirstPoint, point2PointDistanceSecondPoint, point3StraightLengthStart, point4StraightLengthEnd, point5ArcRadiusCenter, point6DiameterCircleCenter, point7AngleFirstVertex, point8AngleFirstEndpoint, point9AngleSecondVertex, point10AngleSecondEndpoint, point11SupportingOffsetSourceStart, point12SupportingOffsetSourceEnd, point13SupportingOffsetTargetStart, point14SupportingOffsetTargetEnd, point15ExactOffsetSourceStart, point16ExactOffsetSourceEnd, point17ExactOffsetTargetStart, point18ExactOffsetTargetEnd]);
  $.group("Geometry", [curve1StraightLengthSpecimen, curve2CircularArcRadiusSpecimen, curve3FullCircleDiameterSpecimen, curve4AngleFirstLeg, curve5AngleSecondLeg, curve6SupportingOffsetSource, curve7SupportingOffsetTarget, curve8ExactOffsetSource, curve9ExactOffsetTarget]);
  $.group("Dimensions", [dimension1PointDistance, dimension2StraightCurveLength, dimension3ArcRadius, dimension4CircleDiameter, dimension5OrientedAngle, dimension6SupportingLineOffset, dimension7ExactTranslatedSegmentOffset]);
  return {};
});
