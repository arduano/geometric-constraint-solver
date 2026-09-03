"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1DirectedAngleFixedVertex = $.geometry.sketchPoint("point1DirectedAngleFixedVertex", {
    point: [0, 0],
    label: "Directed angle fixed vertex",
  });
  const point2DirectedAngleFixedReferenceTip = $.geometry.sketchPoint("point2DirectedAngleFixedReferenceTip", {
    point: [-3.984778792366982, 0.3486229709906328],
    label: "Directed angle fixed reference tip",
  });
  const point3DirectedAngleDraggableBranchCutTip = $.geometry.sketchPoint("point3DirectedAngleDraggableBranchCutTip", {
    point: [-2.988584094275237, -0.2614672282429746],
    label: "Directed angle draggable branch-cut tip",
  });
  const curve1DirectedAngleReferenceRay = $.geometry.segment("curve1DirectedAngleReferenceRay", {
    start: point1DirectedAngleFixedVertex.point,
    end: point2DirectedAngleFixedReferenceTip.point,
    branchDirection: [-0.9961946980917455, 0.0871557427476582],
    label: "Directed angle reference ray",
    role: "profile",
  });
  const curve2DirectedAngleMovingRay = $.geometry.segment("curve2DirectedAngleMovingRay", {
    start: point1DirectedAngleFixedVertex.point,
    end: point3DirectedAngleDraggableBranchCutTip.point,
    branchDirection: [-0.9961946980917454, -0.08715574274765818],
    label: "Directed angle moving ray",
    role: "profile",
  });
  const constraint1DirectedAngleVertexFixed = $.constraint.fixedPoint("constraint1DirectedAngleVertexFixed", {
    point: point1DirectedAngleFixedVertex.point,
    target: [0, 0],
    label: "Directed angle vertex fixed",
  });
  const constraint2DirectedAngleReferenceTipFixed = $.constraint.fixedPoint("constraint2DirectedAngleReferenceTipFixed", {
    point: point2DirectedAngleFixedReferenceTip.point,
    target: [-3.984778792366982, 0.3486229709906328],
    label: "Directed angle reference tip fixed",
  });
  const dimension1DirectedAngleMovingRadius3 = $.dimension.curveLength("dimension1DirectedAngleMovingRadius3", {
    curve: curve2DirectedAngleMovingRay.span,
    value: mm(3),
    label: "Directed angle moving radius 3",
    mode: "driving",
  });
  const dimension2DirectedAngleReferenceBranchCut = $.dimension.orientedAngle("dimension2DirectedAngleReferenceBranchCut", {
    first: curve1DirectedAngleReferenceRay.span,
    second: curve2DirectedAngleMovingRay.span,
    value: rad(0.17453292519943295),
    orientation: "counterClockwise",
    label: "Directed angle reference / branch cut",
    mode: "reference",
  });
  $.group("Points", [point1DirectedAngleFixedVertex, point2DirectedAngleFixedReferenceTip, point3DirectedAngleDraggableBranchCutTip]);
  $.group("Geometry", [curve1DirectedAngleReferenceRay, curve2DirectedAngleMovingRay]);
  $.group("Constraints", [constraint1DirectedAngleVertexFixed, constraint2DirectedAngleReferenceTipFixed]);
  $.group("Dimensions", [dimension1DirectedAngleMovingRadius3, dimension2DirectedAngleReferenceBranchCut]);
  return {};
});
