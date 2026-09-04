"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1LineLineHorizontalStart = $.geometry.sketchPoint("point1LineLineHorizontalStart", {
    point: [-9, 5],
    label: "Line-line horizontal start",
  });
  const point2LineLineHorizontalEnd = $.geometry.sketchPoint("point2LineLineHorizontalEnd", {
    point: [-1, 5],
    label: "Line-line horizontal end",
  });
  const point3LineLineVerticalStart = $.geometry.sketchPoint("point3LineLineVerticalStart", {
    point: [-3, 1],
    label: "Line-line vertical start",
  });
  const point4LineLineVerticalEnd = $.geometry.sketchPoint("point4LineLineVerticalEnd", {
    point: [-3, 9],
    label: "Line-line vertical end",
  });
  const point5HighValenceSharedJunction = $.geometry.sketchPoint("point5HighValenceSharedJunction", {
    point: [14, 6],
    label: "High-valence shared junction",
  });
  const point6HighValenceUpperEndpoint = $.geometry.sketchPoint("point6HighValenceUpperEndpoint", {
    point: [14, 10],
    label: "High-valence upper endpoint",
  });
  const point7HighValenceLowerLeftEndpoint = $.geometry.sketchPoint("point7HighValenceLowerLeftEndpoint", {
    point: [10.5, 3.5],
    label: "High-valence lower-left endpoint",
  });
  const point8HighValenceLowerRightEndpoint = $.geometry.sketchPoint("point8HighValenceLowerRightEndpoint", {
    point: [17.5, 3.5],
    label: "High-valence lower-right endpoint",
  });
  const point9FriendlyLineCircleCenter = $.geometry.sketchPoint("point9FriendlyLineCircleCenter", {
    point: [6, 4],
    label: "Friendly line-circle center",
  });
  const point10FriendlyLineCircleLineStart = $.geometry.sketchPoint("point10FriendlyLineCircleLineStart", {
    point: [1, 2.5],
    label: "Friendly line-circle line start",
  });
  const point11FriendlyLineCircleLineEnd = $.geometry.sketchPoint("point11FriendlyLineCircleLineEnd", {
    point: [11, 2.5],
    label: "Friendly line-circle line end",
  });
  const point12NearFoldStressLineCircleCenter = $.geometry.sketchPoint("point12NearFoldStressLineCircleCenter", {
    point: [22, 7],
    label: "Near-fold stress line-circle center",
  });
  const point13NearFoldStressLineCircleLineStart = $.geometry.sketchPoint("point13NearFoldStressLineCircleLineStart", {
    point: [18, 4],
    label: "Near-fold stress line-circle line start",
  });
  const point14NearFoldStressLineCircleLineEnd = $.geometry.sketchPoint("point14NearFoldStressLineCircleLineEnd", {
    point: [26, 4],
    label: "Near-fold stress line-circle line end",
  });
  const point15LineBezierStart = $.geometry.sketchPoint("point15LineBezierStart", {
    point: [1, -3],
    label: "Line-Bezier start",
  });
  const point16LineBezierControl = $.geometry.sketchPoint("point16LineBezierControl", {
    point: [4, -7],
    label: "Line-Bezier control",
  });
  const point17LineBezierEnd = $.geometry.sketchPoint("point17LineBezierEnd", {
    point: [8, -3],
    label: "Line-Bezier end",
  });
  const point18LineBezierLineStart = $.geometry.sketchPoint("point18LineBezierLineStart", {
    point: [6, -8],
    label: "Line-Bezier line start",
  });
  const point19LineBezierLineEnd = $.geometry.sketchPoint("point19LineBezierLineEnd", {
    point: [6, 0],
    label: "Line-Bezier line end",
  });
  const point20BatchPolylineStart = $.geometry.sketchPoint("point20BatchPolylineStart", {
    point: [-10, -2],
    label: "Batch polyline start",
  });
  const point21BatchPolylineFirstCorner = $.geometry.sketchPoint("point21BatchPolylineFirstCorner", {
    point: [-6, -2],
    label: "Batch polyline first corner",
  });
  const point22BatchPolylineSecondCorner = $.geometry.sketchPoint("point22BatchPolylineSecondCorner", {
    point: [-6, -7],
    label: "Batch polyline second corner",
  });
  const point23BatchPolylineEnd = $.geometry.sketchPoint("point23BatchPolylineEnd", {
    point: [-2, -7],
    label: "Batch polyline end",
  });
  const point24ConflictPolylineStart = $.geometry.sketchPoint("point24ConflictPolylineStart", {
    point: [11, -3],
    label: "Conflict polyline start",
  });
  const point25ConflictPolylineFirstCorner = $.geometry.sketchPoint("point25ConflictPolylineFirstCorner", {
    point: [15, -3],
    label: "Conflict polyline first corner",
  });
  const point26ConflictPolylineSecondCorner = $.geometry.sketchPoint("point26ConflictPolylineSecondCorner", {
    point: [15, -4.75],
    label: "Conflict polyline second corner",
  });
  const point27ConflictPolylineEnd = $.geometry.sketchPoint("point27ConflictPolylineEnd", {
    point: [19, -4.75],
    label: "Conflict polyline end",
  });
  const curve1LineLineHorizontalSupport = $.geometry.segment("curve1LineLineHorizontalSupport", {
    start: point1LineLineHorizontalStart.point,
    end: point2LineLineHorizontalEnd.point,
    branchDirection: [1, 0],
    label: "Line-line horizontal support",
    role: "profile",
  });
  const curve2LineLineVerticalSupport = $.geometry.segment("curve2LineLineVerticalSupport", {
    start: point3LineLineVerticalStart.point,
    end: point4LineLineVerticalEnd.point,
    branchDirection: [0, 1],
    label: "Line-line vertical support",
    role: "profile",
  });
  const curve3HighValenceBranch1 = $.geometry.segment("curve3HighValenceBranch1", {
    start: point5HighValenceSharedJunction.point,
    end: point6HighValenceUpperEndpoint.point,
    branchDirection: [0, 1],
    label: "High-valence branch 1",
    role: "profile",
  });
  const curve4HighValenceBranch2 = $.geometry.segment("curve4HighValenceBranch2", {
    start: point5HighValenceSharedJunction.point,
    end: point7HighValenceLowerLeftEndpoint.point,
    branchDirection: [-0.813733471206735, -0.5812381937190965],
    label: "High-valence branch 2",
    role: "profile",
  });
  const curve5HighValenceBranch3 = $.geometry.segment("curve5HighValenceBranch3", {
    start: point5HighValenceSharedJunction.point,
    end: point8HighValenceLowerRightEndpoint.point,
    branchDirection: [0.813733471206735, -0.5812381937190965],
    label: "High-valence branch 3",
    role: "profile",
  });
  const curve6FriendlyLineCircleCircularSupport = $.geometry.centerRadiusCircle("curve6FriendlyLineCircleCircularSupport", {
    center: point9FriendlyLineCircleCenter.point,
    radius: mm(1),
    label: "Friendly line-circle circular support",
    role: "profile",
  });
  const curve7FriendlyLineCircleLinearSupport = $.geometry.segment("curve7FriendlyLineCircleLinearSupport", {
    start: point10FriendlyLineCircleLineStart.point,
    end: point11FriendlyLineCircleLineEnd.point,
    branchDirection: [1, 0],
    label: "Friendly line-circle linear support",
    role: "profile",
  });
  const curve8NearFoldStressLineCircleCircularSupport = $.geometry.centerRadiusCircle("curve8NearFoldStressLineCircleCircularSupport", {
    center: point12NearFoldStressLineCircleCenter.point,
    radius: mm(2),
    label: "Near-fold stress line-circle circular support",
    role: "profile",
  });
  const curve9NearFoldStressLineCircleLinearSupport = $.geometry.segment("curve9NearFoldStressLineCircleLinearSupport", {
    start: point13NearFoldStressLineCircleLineStart.point,
    end: point14NearFoldStressLineCircleLineEnd.point,
    branchDirection: [1, 0],
    label: "Near-fold stress line-circle linear support",
    role: "profile",
  });
  const curve10LineBezierCurvedSupport = $.geometry.quadraticBezier("curve10LineBezierCurvedSupport", {
    start: point15LineBezierStart.point,
    control: point16LineBezierControl.point,
    end: point17LineBezierEnd.point,
    label: "Line-Bezier curved support",
    role: "profile",
  });
  const curve11LineBezierLinearSupport = $.geometry.segment("curve11LineBezierLinearSupport", {
    start: point18LineBezierLineStart.point,
    end: point19LineBezierLineEnd.point,
    branchDirection: [0, 1],
    label: "Line-Bezier linear support",
    role: "profile",
  });
  const curve12EditableBatchAndSequentialPolyline = $.geometry.polyline("curve12EditableBatchAndSequentialPolyline", {
    vertices: [{
      key: "vertex1",
      position: point20BatchPolylineStart.point,
    }, {
      key: "vertex2",
      position: point21BatchPolylineFirstCorner.point,
    }, {
      key: "vertex3",
      position: point22BatchPolylineSecondCorner.point,
    }, {
      key: "vertex4",
      position: point23BatchPolylineEnd.point,
    }],
    closed: false,
    branchDirections: [[1, 0], [0, -1], [1, 0]],
    label: "Editable batch and sequential polyline",
    role: "profile",
  });
  const curve13EditableShortMiddleConflictPolyline = $.geometry.polyline("curve13EditableShortMiddleConflictPolyline", {
    vertices: [{
      key: "vertex1",
      position: point24ConflictPolylineStart.point,
    }, {
      key: "vertex2",
      position: point25ConflictPolylineFirstCorner.point,
    }, {
      key: "vertex3",
      position: point26ConflictPolylineSecondCorner.point,
    }, {
      key: "vertex4",
      position: point27ConflictPolylineEnd.point,
    }],
    closed: false,
    branchDirections: [[1, 0], [0, -1], [1, 0]],
    label: "Editable short-middle conflict polyline",
    role: "profile",
  });
  const constraint1FixLineLineHorizontalControl1 = $.constraint.fixedPoint("constraint1FixLineLineHorizontalControl1", {
    point: point1LineLineHorizontalStart.point,
    target: [-9, 5],
    label: "Fix Line-line horizontal control 1",
  });
  const constraint2FixLineLineHorizontalControl2 = $.constraint.fixedPoint("constraint2FixLineLineHorizontalControl2", {
    point: point2LineLineHorizontalEnd.point,
    target: [-1, 5],
    label: "Fix Line-line horizontal control 2",
  });
  const constraint3FixLineLineVerticalControl1 = $.constraint.fixedPoint("constraint3FixLineLineVerticalControl1", {
    point: point3LineLineVerticalStart.point,
    target: [-3, 1],
    label: "Fix Line-line vertical control 1",
  });
  const constraint4FixLineLineVerticalControl2 = $.constraint.fixedPoint("constraint4FixLineLineVerticalControl2", {
    point: point4LineLineVerticalEnd.point,
    target: [-3, 9],
    label: "Fix Line-line vertical control 2",
  });
  const constraint5FixHighValenceSharedJunction = $.constraint.fixedPoint("constraint5FixHighValenceSharedJunction", {
    point: point5HighValenceSharedJunction.point,
    target: [14, 6],
    label: "Fix high-valence shared junction",
  });
  const constraint6FixHighValenceEndpoint1 = $.constraint.fixedPoint("constraint6FixHighValenceEndpoint1", {
    point: point6HighValenceUpperEndpoint.point,
    target: [14, 10],
    label: "Fix high-valence endpoint 1",
  });
  const constraint7FixHighValenceEndpoint2 = $.constraint.fixedPoint("constraint7FixHighValenceEndpoint2", {
    point: point7HighValenceLowerLeftEndpoint.point,
    target: [10.5, 3.5],
    label: "Fix high-valence endpoint 2",
  });
  const constraint8FixHighValenceEndpoint3 = $.constraint.fixedPoint("constraint8FixHighValenceEndpoint3", {
    point: point8HighValenceLowerRightEndpoint.point,
    target: [17.5, 3.5],
    label: "Fix high-valence endpoint 3",
  });
  const constraint9FixFriendlyLineCircleCenter = $.constraint.fixedPoint("constraint9FixFriendlyLineCircleCenter", {
    point: point9FriendlyLineCircleCenter.point,
    target: [6, 4],
    label: "Fix Friendly line-circle center",
  });
  const dimension1FriendlyLineCircleSourceRadius = $.dimension.radius("dimension1FriendlyLineCircleSourceRadius", {
    curve: curve6FriendlyLineCircleCircularSupport.curve,
    value: mm(1),
    label: "Friendly line-circle source radius",
    mode: "driving",
  });
  const constraint10FixFriendlyLineCircleLineControl1 = $.constraint.fixedPoint("constraint10FixFriendlyLineCircleLineControl1", {
    point: point10FriendlyLineCircleLineStart.point,
    target: [1, 2.5],
    label: "Fix Friendly line-circle line control 1",
  });
  const constraint11FixFriendlyLineCircleLineControl2 = $.constraint.fixedPoint("constraint11FixFriendlyLineCircleLineControl2", {
    point: point11FriendlyLineCircleLineEnd.point,
    target: [11, 2.5],
    label: "Fix Friendly line-circle line control 2",
  });
  const constraint12FixNearFoldStressLineCircleCenter = $.constraint.fixedPoint("constraint12FixNearFoldStressLineCircleCenter", {
    point: point12NearFoldStressLineCircleCenter.point,
    target: [22, 7],
    label: "Fix Near-fold stress line-circle center",
  });
  const dimension2NearFoldStressLineCircleSourceRadius = $.dimension.radius("dimension2NearFoldStressLineCircleSourceRadius", {
    curve: curve8NearFoldStressLineCircleCircularSupport.curve,
    value: mm(2),
    label: "Near-fold stress line-circle source radius",
    mode: "driving",
  });
  const constraint13FixNearFoldStressLineCircleLineControl1 = $.constraint.fixedPoint("constraint13FixNearFoldStressLineCircleLineControl1", {
    point: point13NearFoldStressLineCircleLineStart.point,
    target: [18, 4],
    label: "Fix Near-fold stress line-circle line control 1",
  });
  const constraint14FixNearFoldStressLineCircleLineControl2 = $.constraint.fixedPoint("constraint14FixNearFoldStressLineCircleLineControl2", {
    point: point14NearFoldStressLineCircleLineEnd.point,
    target: [26, 4],
    label: "Fix Near-fold stress line-circle line control 2",
  });
  const constraint15FixLineBezierCurveControl1 = $.constraint.fixedPoint("constraint15FixLineBezierCurveControl1", {
    point: point15LineBezierStart.point,
    target: [1, -3],
    label: "Fix Line-Bezier curve control 1",
  });
  const constraint16FixLineBezierCurveControl2 = $.constraint.fixedPoint("constraint16FixLineBezierCurveControl2", {
    point: point16LineBezierControl.point,
    target: [4, -7],
    label: "Fix Line-Bezier curve control 2",
  });
  const constraint17FixLineBezierCurveControl3 = $.constraint.fixedPoint("constraint17FixLineBezierCurveControl3", {
    point: point17LineBezierEnd.point,
    target: [8, -3],
    label: "Fix Line-Bezier curve control 3",
  });
  const constraint18FixLineBezierLineControl1 = $.constraint.fixedPoint("constraint18FixLineBezierLineControl1", {
    point: point18LineBezierLineStart.point,
    target: [6, -8],
    label: "Fix Line-Bezier line control 1",
  });
  const constraint19FixLineBezierLineControl2 = $.constraint.fixedPoint("constraint19FixLineBezierLineControl2", {
    point: point19LineBezierLineEnd.point,
    target: [6, 0],
    label: "Fix Line-Bezier line control 2",
  });
  $.group("Points", [point1LineLineHorizontalStart, point2LineLineHorizontalEnd, point3LineLineVerticalStart, point4LineLineVerticalEnd, point5HighValenceSharedJunction, point6HighValenceUpperEndpoint, point7HighValenceLowerLeftEndpoint, point8HighValenceLowerRightEndpoint, point9FriendlyLineCircleCenter, point10FriendlyLineCircleLineStart, point11FriendlyLineCircleLineEnd, point12NearFoldStressLineCircleCenter, point13NearFoldStressLineCircleLineStart, point14NearFoldStressLineCircleLineEnd, point15LineBezierStart, point16LineBezierControl, point17LineBezierEnd, point18LineBezierLineStart, point19LineBezierLineEnd, point20BatchPolylineStart, point21BatchPolylineFirstCorner, point22BatchPolylineSecondCorner, point23BatchPolylineEnd, point24ConflictPolylineStart, point25ConflictPolylineFirstCorner, point26ConflictPolylineSecondCorner, point27ConflictPolylineEnd]);
  $.group("Geometry", [curve1LineLineHorizontalSupport, curve2LineLineVerticalSupport, curve3HighValenceBranch1, curve4HighValenceBranch2, curve5HighValenceBranch3, curve6FriendlyLineCircleCircularSupport, curve7FriendlyLineCircleLinearSupport, curve8NearFoldStressLineCircleCircularSupport, curve9NearFoldStressLineCircleLinearSupport, curve10LineBezierCurvedSupport, curve11LineBezierLinearSupport, curve12EditableBatchAndSequentialPolyline, curve13EditableShortMiddleConflictPolyline]);
  $.group("Constraints", [constraint1FixLineLineHorizontalControl1, constraint2FixLineLineHorizontalControl2, constraint3FixLineLineVerticalControl1, constraint4FixLineLineVerticalControl2, constraint5FixHighValenceSharedJunction, constraint6FixHighValenceEndpoint1, constraint7FixHighValenceEndpoint2, constraint8FixHighValenceEndpoint3, constraint9FixFriendlyLineCircleCenter, constraint10FixFriendlyLineCircleLineControl1, constraint11FixFriendlyLineCircleLineControl2, constraint12FixNearFoldStressLineCircleCenter, constraint13FixNearFoldStressLineCircleLineControl1, constraint14FixNearFoldStressLineCircleLineControl2, constraint15FixLineBezierCurveControl1, constraint16FixLineBezierCurveControl2, constraint17FixLineBezierCurveControl3, constraint18FixLineBezierLineControl1, constraint19FixLineBezierLineControl2]);
  $.group("Dimensions", [dimension1FriendlyLineCircleSourceRadius, dimension2NearFoldStressLineCircleSourceRadius]);
  return {};
});
