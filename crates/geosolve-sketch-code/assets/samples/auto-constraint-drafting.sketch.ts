"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1MidpointReferenceStart = $.geometry.sketchPoint("point1MidpointReferenceStart", {
    point: [-11, 5],
    label: "Midpoint reference start",
  });
  const point2MidpointReferenceEnd = $.geometry.sketchPoint("point2MidpointReferenceEnd", {
    point: [-3, 5],
    label: "Midpoint reference end",
  });
  const point3AffinePolylineStart = $.geometry.sketchPoint("point3AffinePolylineStart", {
    point: [-11, 1],
    label: "Affine polyline start",
  });
  const point4AffinePolylineCorner = $.geometry.sketchPoint("point4AffinePolylineCorner", {
    point: [-7, 3],
    label: "Affine polyline corner",
  });
  const point5AffinePolylineEnd = $.geometry.sketchPoint("point5AffinePolylineEnd", {
    point: [-3, 1],
    label: "Affine polyline end",
  });
  const point6ProfilePointSpecimen = $.geometry.sketchPoint("point6ProfilePointSpecimen", {
    point: [-11, -9],
    label: "Profile point specimen",
  });
  const point7ConstructionPointSpecimen = $.geometry.sketchPoint("point7ConstructionPointSpecimen", {
    point: [-7, -9],
    label: "Construction point specimen",
  });
  const point8RedundantInferenceRejectionStartPoint = $.geometry.sketchPoint("point8RedundantInferenceRejectionStartPoint", {
    point: [2, -9],
    label: "Redundant inference rejection start point",
  });
  const point9RedundantInferenceRejectionEndPoint = $.geometry.sketchPoint("point9RedundantInferenceRejectionEndPoint", {
    point: [10, -9],
    label: "Redundant inference rejection end point",
  });
  const point10CurveTargetCircleCenter = $.geometry.sketchPoint("point10CurveTargetCircleCenter", {
    point: [2, 5],
    label: "Curve target circle center",
  });
  const point11CurveTargetBezierStart = $.geometry.sketchPoint("point11CurveTargetBezierStart", {
    point: [6, 3],
    label: "Curve target Bezier start",
  });
  const point12CurveTargetBezierControl1 = $.geometry.sketchPoint("point12CurveTargetBezierControl1", {
    point: [8, 7],
    label: "Curve target Bezier control 1",
  });
  const point13CurveTargetBezierControl2 = $.geometry.sketchPoint("point13CurveTargetBezierControl2", {
    point: [10, 2],
    label: "Curve target Bezier control 2",
  });
  const point14CurveTargetBezierEnd = $.geometry.sketchPoint("point14CurveTargetBezierEnd", {
    point: [12, 5],
    label: "Curve target Bezier end",
  });
  const point15DraftPoint = $.geometry.sketchPoint("point15DraftPoint", {
    point: [5, -1],
    label: "draft point",
  });
  const point16DraftPoint = $.geometry.sketchPoint("point16DraftPoint", {
    point: [7, -4],
    label: "draft point",
  });
  const point17DraftPoint = $.geometry.sketchPoint("point17DraftPoint", {
    point: [10, 0],
    label: "draft point",
  });
  const point18DraftPoint = $.geometry.sketchPoint("point18DraftPoint", {
    point: [12, -3],
    label: "draft point",
  });
  const point19ProfileOverlapStart = $.geometry.sketchPoint("point19ProfileOverlapStart", {
    point: [-11, -3],
    label: "Profile overlap start",
  });
  const point20ProfileOverlapEnd = $.geometry.sketchPoint("point20ProfileOverlapEnd", {
    point: [-3, -3],
    label: "Profile overlap end",
  });
  const point21ConstructionOverlapStart = $.geometry.sketchPoint("point21ConstructionOverlapStart", {
    point: [-11, -3],
    label: "Construction overlap start",
  });
  const point22ConstructionOverlapEnd = $.geometry.sketchPoint("point22ConstructionOverlapEnd", {
    point: [-3, -3],
    label: "Construction overlap end",
  });
  const point23ExactAmbiguityAStart = $.geometry.sketchPoint("point23ExactAmbiguityAStart", {
    point: [2, -6],
    label: "Exact ambiguity A start",
  });
  const point24ExactAmbiguityAEnd = $.geometry.sketchPoint("point24ExactAmbiguityAEnd", {
    point: [10, -6],
    label: "Exact ambiguity A end",
  });
  const point25ExactAmbiguityBStart = $.geometry.sketchPoint("point25ExactAmbiguityBStart", {
    point: [2, -6],
    label: "Exact ambiguity B start",
  });
  const point26ExactAmbiguityBEnd = $.geometry.sketchPoint("point26ExactAmbiguityBEnd", {
    point: [10, -6],
    label: "Exact ambiguity B end",
  });
  const curve1MidpointAndAffineReferenceLine = $.geometry.segment("curve1MidpointAndAffineReferenceLine", {
    start: point1MidpointReferenceStart.point,
    end: point2MidpointReferenceEnd.point,
    branchDirection: [1, 0],
    label: "Midpoint and affine reference line",
    role: "profile",
  });
  const curve2AffineReferencePolyline = $.geometry.polyline("curve2AffineReferencePolyline", {
    vertices: [{
      key: "vertex1",
      position: point3AffinePolylineStart.point,
    }, {
      key: "vertex2",
      position: point4AffinePolylineCorner.point,
    }, {
      key: "vertex3",
      position: point5AffinePolylineEnd.point,
    }],
    closed: false,
    label: "Affine reference polyline",
    role: "profile",
  });
  const curve3ProfilePointSpecimenMarker = $.geometry.centerRadiusCircle("curve3ProfilePointSpecimenMarker", {
    center: point6ProfilePointSpecimen.point,
    radius: mm(0.75),
    label: "Profile point specimen marker",
    role: "profile",
  });
  const curve4ConstructionPointSpecimenMarker = $.geometry.centerRadiusCircle("curve4ConstructionPointSpecimenMarker", {
    center: point7ConstructionPointSpecimen.point,
    radius: mm(0.75),
    label: "Construction point specimen marker",
    role: "construction",
  });
  const curve5RedundantInferenceRejectionStartMarker = $.geometry.centerRadiusCircle("curve5RedundantInferenceRejectionStartMarker", {
    center: point8RedundantInferenceRejectionStartPoint.point,
    radius: mm(0.65),
    label: "Redundant inference rejection start marker",
    role: "construction",
  });
  const curve6RedundantInferenceRejectionEndMarker = $.geometry.centerRadiusCircle("curve6RedundantInferenceRejectionEndMarker", {
    center: point9RedundantInferenceRejectionEndPoint.point,
    radius: mm(0.65),
    label: "Redundant inference rejection end marker",
    role: "construction",
  });
  const curve7RedundantInferenceRejectionReference = $.geometry.segment("curve7RedundantInferenceRejectionReference", {
    start: point8RedundantInferenceRejectionStartPoint.point,
    end: point9RedundantInferenceRejectionEndPoint.point,
    branchDirection: [1, 0],
    label: "Redundant inference rejection reference",
    role: "construction",
  });
  const curve8CurveTargetCircle = $.geometry.centerRadiusCircle("curve8CurveTargetCircle", {
    center: point10CurveTargetCircleCenter.point,
    radius: mm(2),
    label: "Curve target circle",
    role: "profile",
  });
  const curve9CurveTargetCubicBezier = $.geometry.cubicBezier("curve9CurveTargetCubicBezier", {
    start: point11CurveTargetBezierStart.point,
    firstControl: point12CurveTargetBezierControl1.point,
    secondControl: point13CurveTargetBezierControl2.point,
    end: point14CurveTargetBezierEnd.point,
    label: "Curve target cubic Bezier",
    role: "profile",
  });
  const curve10Nurbs = $.geometry.openControlNurbs("curve10Nurbs", {
    controls: [{
      key: "control1",
      position: point15DraftPoint.point,
      weight: 1,
    }, {
      key: "control2",
      position: point16DraftPoint.point,
      weight: 1,
    }, {
      key: "control3",
      position: point17DraftPoint.point,
      weight: 1,
    }, {
      key: "control4",
      position: point18DraftPoint.point,
      weight: 1,
    }],
    degree: 3,
    gauge: "control1",
    label: "NURBS",
    role: "profile",
  });
  const curve11ProfileOverlapPriorityReference = $.geometry.segment("curve11ProfileOverlapPriorityReference", {
    start: point19ProfileOverlapStart.point,
    end: point20ProfileOverlapEnd.point,
    branchDirection: [1, 0],
    label: "Profile overlap priority reference",
    role: "profile",
  });
  const curve12ConstructionOverlapPriorityReference = $.geometry.segment("curve12ConstructionOverlapPriorityReference", {
    start: point21ConstructionOverlapStart.point,
    end: point22ConstructionOverlapEnd.point,
    branchDirection: [1, 0],
    label: "Construction overlap priority reference",
    role: "construction",
  });
  const curve13ExactAmbiguityReferenceA = $.geometry.segment("curve13ExactAmbiguityReferenceA", {
    start: point23ExactAmbiguityAStart.point,
    end: point24ExactAmbiguityAEnd.point,
    branchDirection: [1, 0],
    label: "Exact ambiguity reference A",
    role: "profile",
  });
  const curve14ExactAmbiguityReferenceB = $.geometry.segment("curve14ExactAmbiguityReferenceB", {
    start: point25ExactAmbiguityBStart.point,
    end: point26ExactAmbiguityBEnd.point,
    branchDirection: [1, 0],
    label: "Exact ambiguity reference B",
    role: "profile",
  });
  const constraint1FixMidpointReferenceControl1 = $.constraint.fixedPoint("constraint1FixMidpointReferenceControl1", {
    point: point1MidpointReferenceStart.point,
    target: [-11, 5],
    label: "Fix midpoint reference control 1",
  });
  const constraint2FixMidpointReferenceControl2 = $.constraint.fixedPoint("constraint2FixMidpointReferenceControl2", {
    point: point2MidpointReferenceEnd.point,
    target: [-3, 5],
    label: "Fix midpoint reference control 2",
  });
  const constraint3FixAffineReferencePolylineControl1 = $.constraint.fixedPoint("constraint3FixAffineReferencePolylineControl1", {
    point: point3AffinePolylineStart.point,
    target: [-11, 1],
    label: "Fix affine reference polyline control 1",
  });
  const constraint4FixAffineReferencePolylineControl2 = $.constraint.fixedPoint("constraint4FixAffineReferencePolylineControl2", {
    point: point4AffinePolylineCorner.point,
    target: [-7, 3],
    label: "Fix affine reference polyline control 2",
  });
  const constraint5FixAffineReferencePolylineControl3 = $.constraint.fixedPoint("constraint5FixAffineReferencePolylineControl3", {
    point: point5AffinePolylineEnd.point,
    target: [-3, 1],
    label: "Fix affine reference polyline control 3",
  });
  const constraint6FixConstructionPointSpecimen = $.constraint.fixedPoint("constraint6FixConstructionPointSpecimen", {
    point: point7ConstructionPointSpecimen.point,
    target: [-7, -9],
    label: "Fix Construction point specimen",
  });
  const constraint7PreparedRejectionReferenceIsHorizontal = $.constraint.horizontal("constraint7PreparedRejectionReferenceIsHorizontal", {
    span: curve7RedundantInferenceRejectionReference.span,
    label: "Prepared rejection reference is horizontal",
  });
  const constraint8FixCurveTargetCircleCenter = $.constraint.fixedPoint("constraint8FixCurveTargetCircleCenter", {
    point: point10CurveTargetCircleCenter.point,
    target: [2, 5],
    label: "Fix curve target circle center",
  });
  const dimension1CurveTargetCircleRadiusDimension = $.dimension.radius("dimension1CurveTargetCircleRadiusDimension", {
    curve: curve8CurveTargetCircle.curve,
    value: mm(2),
    label: "Curve target circle radius dimension",
    mode: "driving",
  });
  const constraint9FixCurveTargetCubicBezierControl1 = $.constraint.fixedPoint("constraint9FixCurveTargetCubicBezierControl1", {
    point: point11CurveTargetBezierStart.point,
    target: [6, 3],
    label: "Fix curve target cubic Bezier control 1",
  });
  const constraint10FixCurveTargetCubicBezierControl2 = $.constraint.fixedPoint("constraint10FixCurveTargetCubicBezierControl2", {
    point: point12CurveTargetBezierControl1.point,
    target: [8, 7],
    label: "Fix curve target cubic Bezier control 2",
  });
  const constraint11FixCurveTargetCubicBezierControl3 = $.constraint.fixedPoint("constraint11FixCurveTargetCubicBezierControl3", {
    point: point13CurveTargetBezierControl2.point,
    target: [10, 2],
    label: "Fix curve target cubic Bezier control 3",
  });
  const constraint12FixCurveTargetCubicBezierControl4 = $.constraint.fixedPoint("constraint12FixCurveTargetCubicBezierControl4", {
    point: point14CurveTargetBezierEnd.point,
    target: [12, 5],
    label: "Fix curve target cubic Bezier control 4",
  });
  const constraint13FixCurveTargetNurbsControl1 = $.constraint.fixedPoint("constraint13FixCurveTargetNurbsControl1", {
    point: point15DraftPoint.point,
    target: [5, -1],
    label: "Fix curve target NURBS control 1",
  });
  const constraint14FixCurveTargetNurbsControl2 = $.constraint.fixedPoint("constraint14FixCurveTargetNurbsControl2", {
    point: point16DraftPoint.point,
    target: [7, -4],
    label: "Fix curve target NURBS control 2",
  });
  const constraint15FixCurveTargetNurbsControl3 = $.constraint.fixedPoint("constraint15FixCurveTargetNurbsControl3", {
    point: point17DraftPoint.point,
    target: [10, 0],
    label: "Fix curve target NURBS control 3",
  });
  const constraint16FixCurveTargetNurbsControl4 = $.constraint.fixedPoint("constraint16FixCurveTargetNurbsControl4", {
    point: point18DraftPoint.point,
    target: [12, -3],
    label: "Fix curve target NURBS control 4",
  });
  const constraint17FixProfileOverlapReferenceControl1 = $.constraint.fixedPoint("constraint17FixProfileOverlapReferenceControl1", {
    point: point19ProfileOverlapStart.point,
    target: [-11, -3],
    label: "Fix profile overlap reference control 1",
  });
  const constraint18FixProfileOverlapReferenceControl2 = $.constraint.fixedPoint("constraint18FixProfileOverlapReferenceControl2", {
    point: point20ProfileOverlapEnd.point,
    target: [-3, -3],
    label: "Fix profile overlap reference control 2",
  });
  const constraint19FixConstructionOverlapReferenceControl1 = $.constraint.fixedPoint("constraint19FixConstructionOverlapReferenceControl1", {
    point: point21ConstructionOverlapStart.point,
    target: [-11, -3],
    label: "Fix construction overlap reference control 1",
  });
  const constraint20FixConstructionOverlapReferenceControl2 = $.constraint.fixedPoint("constraint20FixConstructionOverlapReferenceControl2", {
    point: point22ConstructionOverlapEnd.point,
    target: [-3, -3],
    label: "Fix construction overlap reference control 2",
  });
  const constraint21FixExactAmbiguityReferenceAControl1 = $.constraint.fixedPoint("constraint21FixExactAmbiguityReferenceAControl1", {
    point: point23ExactAmbiguityAStart.point,
    target: [2, -6],
    label: "Fix exact ambiguity reference A control 1",
  });
  const constraint22FixExactAmbiguityReferenceAControl2 = $.constraint.fixedPoint("constraint22FixExactAmbiguityReferenceAControl2", {
    point: point24ExactAmbiguityAEnd.point,
    target: [10, -6],
    label: "Fix exact ambiguity reference A control 2",
  });
  const constraint23FixExactAmbiguityReferenceBControl1 = $.constraint.fixedPoint("constraint23FixExactAmbiguityReferenceBControl1", {
    point: point25ExactAmbiguityBStart.point,
    target: [2, -6],
    label: "Fix exact ambiguity reference B control 1",
  });
  const constraint24FixExactAmbiguityReferenceBControl2 = $.constraint.fixedPoint("constraint24FixExactAmbiguityReferenceBControl2", {
    point: point26ExactAmbiguityBEnd.point,
    target: [10, -6],
    label: "Fix exact ambiguity reference B control 2",
  });
  $.group("Points", [point1MidpointReferenceStart, point2MidpointReferenceEnd, point3AffinePolylineStart, point4AffinePolylineCorner, point5AffinePolylineEnd, point6ProfilePointSpecimen, point7ConstructionPointSpecimen, point8RedundantInferenceRejectionStartPoint, point9RedundantInferenceRejectionEndPoint, point10CurveTargetCircleCenter, point11CurveTargetBezierStart, point12CurveTargetBezierControl1, point13CurveTargetBezierControl2, point14CurveTargetBezierEnd, point15DraftPoint, point16DraftPoint, point17DraftPoint, point18DraftPoint, point19ProfileOverlapStart, point20ProfileOverlapEnd, point21ConstructionOverlapStart, point22ConstructionOverlapEnd, point23ExactAmbiguityAStart, point24ExactAmbiguityAEnd, point25ExactAmbiguityBStart, point26ExactAmbiguityBEnd]);
  $.group("Geometry", [curve1MidpointAndAffineReferenceLine, curve2AffineReferencePolyline, curve3ProfilePointSpecimenMarker, curve4ConstructionPointSpecimenMarker, curve5RedundantInferenceRejectionStartMarker, curve6RedundantInferenceRejectionEndMarker, curve7RedundantInferenceRejectionReference, curve8CurveTargetCircle, curve9CurveTargetCubicBezier, curve10Nurbs, curve11ProfileOverlapPriorityReference, curve12ConstructionOverlapPriorityReference, curve13ExactAmbiguityReferenceA, curve14ExactAmbiguityReferenceB]);
  $.group("Constraints", [constraint1FixMidpointReferenceControl1, constraint2FixMidpointReferenceControl2, constraint3FixAffineReferencePolylineControl1, constraint4FixAffineReferencePolylineControl2, constraint5FixAffineReferencePolylineControl3, constraint6FixConstructionPointSpecimen, constraint7PreparedRejectionReferenceIsHorizontal, constraint8FixCurveTargetCircleCenter, constraint9FixCurveTargetCubicBezierControl1, constraint10FixCurveTargetCubicBezierControl2, constraint11FixCurveTargetCubicBezierControl3, constraint12FixCurveTargetCubicBezierControl4, constraint13FixCurveTargetNurbsControl1, constraint14FixCurveTargetNurbsControl2, constraint15FixCurveTargetNurbsControl3, constraint16FixCurveTargetNurbsControl4, constraint17FixProfileOverlapReferenceControl1, constraint18FixProfileOverlapReferenceControl2, constraint19FixConstructionOverlapReferenceControl1, constraint20FixConstructionOverlapReferenceControl2, constraint21FixExactAmbiguityReferenceAControl1, constraint22FixExactAmbiguityReferenceAControl2, constraint23FixExactAmbiguityReferenceBControl1, constraint24FixExactAmbiguityReferenceBControl2]);
  $.group("Dimensions", [dimension1CurveTargetCircleRadiusDimension]);
  return {};
});
