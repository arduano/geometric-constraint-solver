"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1ProfilePolylinePoint = $.geometry.sketchPoint("point1ProfilePolylinePoint", {
    point: [0, 0],
    label: "Profile polyline point",
  });
  const point2ProfilePolylinePoint = $.geometry.sketchPoint("point2ProfilePolylinePoint", {
    point: [2, 0],
    label: "Profile polyline point",
  });
  const point3ProfilePolylinePoint = $.geometry.sketchPoint("point3ProfilePolylinePoint", {
    point: [1, 2],
    label: "Profile polyline point",
  });
  const point4ProfileLinePoint = $.geometry.sketchPoint("point4ProfileLinePoint", {
    point: [10, 0],
    label: "Profile line point",
  });
  const point5ProfileLinePoint = $.geometry.sketchPoint("point5ProfileLinePoint", {
    point: [12, 0],
    label: "Profile line point",
  });
  const point6ProfileLinePoint = $.geometry.sketchPoint("point6ProfileLinePoint", {
    point: [12, 2],
    label: "Profile line point",
  });
  const point7ProfileLinePoint = $.geometry.sketchPoint("point7ProfileLinePoint", {
    point: [10, 2],
    label: "Profile line point",
  });
  const point8ProfileCircleCenter = $.geometry.sketchPoint("point8ProfileCircleCenter", {
    point: [21, 1],
    label: "Profile circle center",
  });
  const point9ProfileEllipseCenter = $.geometry.sketchPoint("point9ProfileEllipseCenter", {
    point: [31, 1],
    label: "Profile ellipse center",
  });
  const point10ProfileEllipseAxis = $.geometry.sketchPoint("point10ProfileEllipseAxis", {
    point: [33, 1],
    label: "Profile ellipse axis",
  });
  const point11ProfileQuadraticControl = $.geometry.sketchPoint("point11ProfileQuadraticControl", {
    point: [40, 0],
    label: "Profile quadratic control",
  });
  const point12ProfileQuadraticControl = $.geometry.sketchPoint("point12ProfileQuadraticControl", {
    point: [41, 3],
    label: "Profile quadratic control",
  });
  const point13ProfileQuadraticControl = $.geometry.sketchPoint("point13ProfileQuadraticControl", {
    point: [42, 0],
    label: "Profile quadratic control",
  });
  const point14ProfileCubicControl = $.geometry.sketchPoint("point14ProfileCubicControl", {
    point: [50, 0],
    label: "Profile cubic control",
  });
  const point15ProfileCubicControl = $.geometry.sketchPoint("point15ProfileCubicControl", {
    point: [50.5, 3],
    label: "Profile cubic control",
  });
  const point16ProfileCubicControl = $.geometry.sketchPoint("point16ProfileCubicControl", {
    point: [51.5, 3],
    label: "Profile cubic control",
  });
  const point17ProfileCubicControl = $.geometry.sketchPoint("point17ProfileCubicControl", {
    point: [52, 0],
    label: "Profile cubic control",
  });
  const point18ProfileRationalStart = $.geometry.sketchPoint("point18ProfileRationalStart", {
    point: [60, 0],
    label: "Profile rational start",
  });
  const point19ProfileRationalEnd = $.geometry.sketchPoint("point19ProfileRationalEnd", {
    point: [62, 0],
    label: "Profile rational end",
  });
  const point20ProfileCircularArcCenter = $.geometry.sketchPoint("point20ProfileCircularArcCenter", {
    point: [71, 0],
    label: "Profile circular arc center",
  });
  const point21ProfileCircularArcFirstEndpoint = $.geometry.sketchPoint("point21ProfileCircularArcFirstEndpoint", {
    point: [73, 0],
    label: "Profile circular arc first endpoint",
  });
  const point22ProfileCircularArcSecondEndpoint = $.geometry.sketchPoint("point22ProfileCircularArcSecondEndpoint", {
    point: [69, 2.4492935982947064e-16],
    label: "Profile circular arc second endpoint",
  });
  const point23ProfileEllipticalArcCenter = $.geometry.sketchPoint("point23ProfileEllipticalArcCenter", {
    point: [81, 0],
    label: "Profile elliptical arc center",
  });
  const point24ProfileEllipticalArcAxis = $.geometry.sketchPoint("point24ProfileEllipticalArcAxis", {
    point: [83, 0],
    label: "Profile elliptical arc axis",
  });
  const point25ProfileEllipticalArcFirstEndpoint = $.geometry.sketchPoint("point25ProfileEllipticalArcFirstEndpoint", {
    point: [83, 0],
    label: "Profile elliptical arc first endpoint",
  });
  const point26ProfileEllipticalArcSecondEndpoint = $.geometry.sketchPoint("point26ProfileEllipticalArcSecondEndpoint", {
    point: [79, 1.2246467991473532e-16],
    label: "Profile elliptical arc second endpoint",
  });
  const point27ProfileParabolaVertex = $.geometry.sketchPoint("point27ProfileParabolaVertex", {
    point: [91, 0],
    label: "Profile parabola vertex",
  });
  const point28ProfileParabolaFocus = $.geometry.sketchPoint("point28ProfileParabolaFocus", {
    point: [91, 0.5],
    label: "Profile parabola focus",
  });
  const point29ProfileParabolaFirstEndpoint = $.geometry.sketchPoint("point29ProfileParabolaFirstEndpoint", {
    point: [93, 2],
    label: "Profile parabola first endpoint",
  });
  const point30ProfileParabolaSecondEndpoint = $.geometry.sketchPoint("point30ProfileParabolaSecondEndpoint", {
    point: [89, 2],
    label: "Profile parabola second endpoint",
  });
  const point31ProfileHyperbolaCenter = $.geometry.sketchPoint("point31ProfileHyperbolaCenter", {
    point: [101, 0],
    label: "Profile hyperbola center",
  });
  const point32ProfileHyperbolaAxis = $.geometry.sketchPoint("point32ProfileHyperbolaAxis", {
    point: [102, 0],
    label: "Profile hyperbola axis",
  });
  const point33ProfileHyperbolaFirstEndpoint = $.geometry.sketchPoint("point33ProfileHyperbolaFirstEndpoint", {
    point: [102.54308063481524, -1.1752011936438014],
    label: "Profile hyperbola first endpoint",
  });
  const point34ProfileHyperbolaSecondEndpoint = $.geometry.sketchPoint("point34ProfileHyperbolaSecondEndpoint", {
    point: [102.54308063481524, 1.1752011936438014],
    label: "Profile hyperbola second endpoint",
  });
  const point35ProfileSplineControl = $.geometry.sketchPoint("point35ProfileSplineControl", {
    point: [110, 0],
    label: "Profile spline control",
  });
  const point36ProfileSplineControl = $.geometry.sketchPoint("point36ProfileSplineControl", {
    point: [111, 3],
    label: "Profile spline control",
  });
  const point37ProfileSplineControl = $.geometry.sketchPoint("point37ProfileSplineControl", {
    point: [112, 0],
    label: "Profile spline control",
  });
  const point38ProfileSplineControl = $.geometry.sketchPoint("point38ProfileSplineControl", {
    point: [120, 0],
    label: "Profile spline control",
  });
  const point39ProfileSplineControl = $.geometry.sketchPoint("point39ProfileSplineControl", {
    point: [121, -1],
    label: "Profile spline control",
  });
  const point40ProfileSplineControl = $.geometry.sketchPoint("point40ProfileSplineControl", {
    point: [122, 0],
    label: "Profile spline control",
  });
  const point41ProfileSplineControl = $.geometry.sketchPoint("point41ProfileSplineControl", {
    point: [121.5, 2],
    label: "Profile spline control",
  });
  const point42ProfileSplineControl = $.geometry.sketchPoint("point42ProfileSplineControl", {
    point: [120.5, 2],
    label: "Profile spline control",
  });
  const point43ProfileSplineControl = $.geometry.sketchPoint("point43ProfileSplineControl", {
    point: [130, 0],
    label: "Profile spline control",
  });
  const point44ProfileSplineControl = $.geometry.sketchPoint("point44ProfileSplineControl", {
    point: [131, 3],
    label: "Profile spline control",
  });
  const point45ProfileSplineControl = $.geometry.sketchPoint("point45ProfileSplineControl", {
    point: [132, 0],
    label: "Profile spline control",
  });
  const point46ProfileSplineControl = $.geometry.sketchPoint("point46ProfileSplineControl", {
    point: [140, 0],
    label: "Profile spline control",
  });
  const point47ProfileSplineControl = $.geometry.sketchPoint("point47ProfileSplineControl", {
    point: [141, -1],
    label: "Profile spline control",
  });
  const point48ProfileSplineControl = $.geometry.sketchPoint("point48ProfileSplineControl", {
    point: [142, 0],
    label: "Profile spline control",
  });
  const point49ProfileSplineControl = $.geometry.sketchPoint("point49ProfileSplineControl", {
    point: [141.5, 2],
    label: "Profile spline control",
  });
  const point50ProfileSplineControl = $.geometry.sketchPoint("point50ProfileSplineControl", {
    point: [140.5, 2],
    label: "Profile spline control",
  });
  const point51ProfileSplitterStart = $.geometry.sketchPoint("point51ProfileSplitterStart", {
    point: [1, -4],
    label: "Profile splitter start",
  });
  const point52ProfileSplitterEnd = $.geometry.sketchPoint("point52ProfileSplitterEnd", {
    point: [1, 4],
    label: "Profile splitter end",
  });
  const point53ProfileSplitterStart = $.geometry.sketchPoint("point53ProfileSplitterStart", {
    point: [11, -4],
    label: "Profile splitter start",
  });
  const point54ProfileSplitterEnd = $.geometry.sketchPoint("point54ProfileSplitterEnd", {
    point: [11, 4],
    label: "Profile splitter end",
  });
  const point55ProfileSplitterStart = $.geometry.sketchPoint("point55ProfileSplitterStart", {
    point: [21, -4],
    label: "Profile splitter start",
  });
  const point56ProfileSplitterEnd = $.geometry.sketchPoint("point56ProfileSplitterEnd", {
    point: [21, 4],
    label: "Profile splitter end",
  });
  const point57ProfileSplitterStart = $.geometry.sketchPoint("point57ProfileSplitterStart", {
    point: [31, -4],
    label: "Profile splitter start",
  });
  const point58ProfileSplitterEnd = $.geometry.sketchPoint("point58ProfileSplitterEnd", {
    point: [31, 4],
    label: "Profile splitter end",
  });
  const point59ProfileSplitterStart = $.geometry.sketchPoint("point59ProfileSplitterStart", {
    point: [41, -4],
    label: "Profile splitter start",
  });
  const point60ProfileSplitterEnd = $.geometry.sketchPoint("point60ProfileSplitterEnd", {
    point: [41, 4],
    label: "Profile splitter end",
  });
  const point61ProfileSplitterStart = $.geometry.sketchPoint("point61ProfileSplitterStart", {
    point: [51, -4],
    label: "Profile splitter start",
  });
  const point62ProfileSplitterEnd = $.geometry.sketchPoint("point62ProfileSplitterEnd", {
    point: [51, 4],
    label: "Profile splitter end",
  });
  const point63ProfileSplitterStart = $.geometry.sketchPoint("point63ProfileSplitterStart", {
    point: [61, -4],
    label: "Profile splitter start",
  });
  const point64ProfileSplitterEnd = $.geometry.sketchPoint("point64ProfileSplitterEnd", {
    point: [61, 4],
    label: "Profile splitter end",
  });
  const point65ProfileSplitterStart = $.geometry.sketchPoint("point65ProfileSplitterStart", {
    point: [71, -4],
    label: "Profile splitter start",
  });
  const point66ProfileSplitterEnd = $.geometry.sketchPoint("point66ProfileSplitterEnd", {
    point: [71, 4],
    label: "Profile splitter end",
  });
  const point67ProfileSplitterStart = $.geometry.sketchPoint("point67ProfileSplitterStart", {
    point: [81, -4],
    label: "Profile splitter start",
  });
  const point68ProfileSplitterEnd = $.geometry.sketchPoint("point68ProfileSplitterEnd", {
    point: [81, 4],
    label: "Profile splitter end",
  });
  const point69ProfileSplitterStart = $.geometry.sketchPoint("point69ProfileSplitterStart", {
    point: [91, -4],
    label: "Profile splitter start",
  });
  const point70ProfileSplitterEnd = $.geometry.sketchPoint("point70ProfileSplitterEnd", {
    point: [91, 4],
    label: "Profile splitter end",
  });
  const point71ProfileSplitterStart = $.geometry.sketchPoint("point71ProfileSplitterStart", {
    point: [111, -4],
    label: "Profile splitter start",
  });
  const point72ProfileSplitterEnd = $.geometry.sketchPoint("point72ProfileSplitterEnd", {
    point: [111, 4],
    label: "Profile splitter end",
  });
  const point73ProfileSplitterStart = $.geometry.sketchPoint("point73ProfileSplitterStart", {
    point: [121.2, -4],
    label: "Profile splitter start",
  });
  const point74ProfileSplitterEnd = $.geometry.sketchPoint("point74ProfileSplitterEnd", {
    point: [121.2, 4],
    label: "Profile splitter end",
  });
  const point75ProfileSplitterStart = $.geometry.sketchPoint("point75ProfileSplitterStart", {
    point: [131, -4],
    label: "Profile splitter start",
  });
  const point76ProfileSplitterEnd = $.geometry.sketchPoint("point76ProfileSplitterEnd", {
    point: [131, 4],
    label: "Profile splitter end",
  });
  const point77ProfileSplitterStart = $.geometry.sketchPoint("point77ProfileSplitterStart", {
    point: [141.2, -4],
    label: "Profile splitter start",
  });
  const point78ProfileSplitterEnd = $.geometry.sketchPoint("point78ProfileSplitterEnd", {
    point: [141.2, 4],
    label: "Profile splitter end",
  });
  const point79ProfileHyperbolaSplitterStart = $.geometry.sketchPoint("point79ProfileHyperbolaSplitterStart", {
    point: [99, 0],
    label: "Profile hyperbola splitter start",
  });
  const point80ProfileHyperbolaSplitterEnd = $.geometry.sketchPoint("point80ProfileHyperbolaSplitterEnd", {
    point: [105, 0],
    label: "Profile hyperbola splitter end",
  });
  const curve1ProfileClosedPolyline = $.geometry.polyline("curve1ProfileClosedPolyline", {
    vertices: [{
      key: "vertex1",
      position: point1ProfilePolylinePoint.point,
    }, {
      key: "vertex2",
      position: point2ProfilePolylinePoint.point,
    }, {
      key: "vertex3",
      position: point3ProfilePolylinePoint.point,
    }],
    closed: true,
    label: "Profile closed polyline",
    role: "profile",
  });
  const curve2ProfileLineSquareEdge = $.geometry.segment("curve2ProfileLineSquareEdge", {
    start: point4ProfileLinePoint.point,
    end: point5ProfileLinePoint.point,
    branchDirection: [1, 0],
    label: "Profile line square edge",
    role: "profile",
  });
  const curve3ProfileLineSquareEdge = $.geometry.segment("curve3ProfileLineSquareEdge", {
    start: point5ProfileLinePoint.point,
    end: point6ProfileLinePoint.point,
    branchDirection: [0, 1],
    label: "Profile line square edge",
    role: "profile",
  });
  const curve4ProfileLineSquareEdge = $.geometry.segment("curve4ProfileLineSquareEdge", {
    start: point6ProfileLinePoint.point,
    end: point7ProfileLinePoint.point,
    branchDirection: [-1, 0],
    label: "Profile line square edge",
    role: "profile",
  });
  const curve5ProfileLineSquareEdge = $.geometry.segment("curve5ProfileLineSquareEdge", {
    start: point7ProfileLinePoint.point,
    end: point4ProfileLinePoint.point,
    branchDirection: [0, -1],
    label: "Profile line square edge",
    role: "profile",
  });
  const curve6ProfileCircle = $.geometry.centerRadiusCircle("curve6ProfileCircle", {
    center: point8ProfileCircleCenter.point,
    radius: mm(1),
    label: "Profile circle",
    role: "profile",
  });
  const curve7ProfileEllipse = $.geometry.centerAxesEllipse("curve7ProfileEllipse", {
    center: point9ProfileEllipseCenter.point,
    majorAxisPoint: point10ProfileEllipseAxis.point,
    minorAxisPoint: [31, 2],
    label: "Profile ellipse",
    role: "profile",
  });
  const curve8ProfileQuadraticBezier = $.geometry.quadraticBezier("curve8ProfileQuadraticBezier", {
    start: point11ProfileQuadraticControl.point,
    control: point12ProfileQuadraticControl.point,
    end: point13ProfileQuadraticControl.point,
    label: "Profile quadratic Bezier",
    role: "profile",
  });
  const curve9ProfileQuadraticClosure = $.geometry.segment("curve9ProfileQuadraticClosure", {
    start: point13ProfileQuadraticControl.point,
    end: point11ProfileQuadraticControl.point,
    branchDirection: [-1, 0],
    label: "Profile quadratic closure",
    role: "profile",
  });
  const curve10ProfileCubicBezier = $.geometry.cubicBezier("curve10ProfileCubicBezier", {
    start: point14ProfileCubicControl.point,
    firstControl: point15ProfileCubicControl.point,
    secondControl: point16ProfileCubicControl.point,
    end: point17ProfileCubicControl.point,
    label: "Profile cubic Bezier",
    role: "profile",
  });
  const curve11ProfileCubicClosure = $.geometry.segment("curve11ProfileCubicClosure", {
    start: point17ProfileCubicControl.point,
    end: point14ProfileCubicControl.point,
    branchDirection: [-1, 0],
    label: "Profile cubic closure",
    role: "profile",
  });
  const curve12ProfileRationalQuadraticConic = $.geometry.rationalQuadraticConic("curve12ProfileRationalQuadraticConic", {
    start: point18ProfileRationalStart.point,
    end: point19ProfileRationalEnd.point,
    weightedMiddle: [45.75, 2.25],
    middleWeight: 0.75,
    label: "Profile rational quadratic conic",
    role: "profile",
  });
  const curve13ProfileRationalClosure = $.geometry.segment("curve13ProfileRationalClosure", {
    start: point19ProfileRationalEnd.point,
    end: point18ProfileRationalStart.point,
    branchDirection: [-1, 0],
    label: "Profile rational closure",
    role: "profile",
  });
  const curve14ProfileCircularArc = $.geometry.centerArc("curve14ProfileCircularArc", {
    center: point20ProfileCircularArcCenter.point,
    start: [73, 0],
    end: [69, 2.4492935982947064e-16],
    sweep: "counterClockwise",
    label: "Profile circular arc",
    role: "profile",
  });
  const curve15ProfileCircularArcClosure = $.geometry.segment("curve15ProfileCircularArcClosure", {
    start: point22ProfileCircularArcSecondEndpoint.point,
    end: point21ProfileCircularArcFirstEndpoint.point,
    branchDirection: [1, -6.123233995736766e-17],
    label: "Profile circular arc closure",
    role: "profile",
  });
  const curve16ProfileEllipticalArc = $.geometry.centerAxesEllipticalArc("curve16ProfileEllipticalArc", {
    center: point23ProfileEllipticalArcCenter.point,
    majorAxisPoint: point24ProfileEllipticalArcAxis.point,
    minorAxisPoint: [81, 1],
    start: [83, 0],
    end: [79, 1.2246467991473532e-16],
    sweep: "counterClockwise",
    label: "Profile elliptical arc",
    role: "profile",
  });
  const curve17ProfileEllipticalArcClosure = $.geometry.segment("curve17ProfileEllipticalArcClosure", {
    start: point26ProfileEllipticalArcSecondEndpoint.point,
    end: point25ProfileEllipticalArcFirstEndpoint.point,
    branchDirection: [1, -3.061616997868383e-17],
    label: "Profile elliptical arc closure",
    role: "profile",
  });
  const curve18ProfileParabola = $.geometry.parabola("curve18ProfileParabola", {
    vertex: point27ProfileParabolaVertex.point,
    focus: point28ProfileParabolaFocus.point,
    trimStart: -2,
    trimEnd: 2,
    label: "Profile parabola",
    role: "profile",
  });
  const curve19ProfileParabolaClosure = $.geometry.segment("curve19ProfileParabolaClosure", {
    start: point30ProfileParabolaSecondEndpoint.point,
    end: point29ProfileParabolaFirstEndpoint.point,
    branchDirection: [1, 0],
    label: "Profile parabola closure",
    role: "profile",
  });
  const curve20ProfileHyperbola = $.geometry.hyperbola("curve20ProfileHyperbola", {
    center: point31ProfileHyperbolaCenter.point,
    transverseAxisPoint: point32ProfileHyperbolaAxis.point,
    semiConjugate: mm(1),
    trimStart: -1,
    trimEnd: 1,
    branch: "positive",
    label: "Profile hyperbola",
    role: "profile",
  });
  const curve21ProfileHyperbolaClosure = $.geometry.segment("curve21ProfileHyperbolaClosure", {
    start: point34ProfileHyperbolaSecondEndpoint.point,
    end: point33ProfileHyperbolaFirstEndpoint.point,
    branchDirection: [0, -1],
    label: "Profile hyperbola closure",
    role: "profile",
  });
  const curve22ProfileBSpline = $.geometry.openControlBSpline("curve22ProfileBSpline", {
    controls: [{
      key: "control1",
      position: point35ProfileSplineControl.point,
    }, {
      key: "control2",
      position: point36ProfileSplineControl.point,
    }, {
      key: "control3",
      position: point37ProfileSplineControl.point,
    }],
    degree: 2,
    label: "Profile B-spline",
    role: "profile",
  });
  const curve23ProfileSplineClosure = $.geometry.segment("curve23ProfileSplineClosure", {
    start: point37ProfileSplineControl.point,
    end: point35ProfileSplineControl.point,
    branchDirection: [-1, 0],
    label: "Profile spline closure",
    role: "profile",
  });
  const curve24ProfileBSpline = $.geometry.periodicControlBSpline("curve24ProfileBSpline", {
    controls: [{
      key: "control1",
      position: point38ProfileSplineControl.point,
    }, {
      key: "control2",
      position: point39ProfileSplineControl.point,
    }, {
      key: "control3",
      position: point40ProfileSplineControl.point,
    }, {
      key: "control4",
      position: point41ProfileSplineControl.point,
    }, {
      key: "control5",
      position: point42ProfileSplineControl.point,
    }],
    degree: 2,
    label: "Profile B-spline",
    role: "profile",
  });
  const curve25ProfileNurbs = $.geometry.openControlNurbs("curve25ProfileNurbs", {
    controls: [{
      key: "control1",
      position: point43ProfileSplineControl.point,
      weight: 1,
    }, {
      key: "control2",
      position: point44ProfileSplineControl.point,
      weight: 0.8,
    }, {
      key: "control3",
      position: point45ProfileSplineControl.point,
      weight: 1,
    }],
    degree: 2,
    gauge: "control1",
    label: "Profile NURBS",
    role: "profile",
  });
  const curve26ProfileSplineClosure = $.geometry.segment("curve26ProfileSplineClosure", {
    start: point45ProfileSplineControl.point,
    end: point43ProfileSplineControl.point,
    branchDirection: [-1, 0],
    label: "Profile spline closure",
    role: "profile",
  });
  const curve27ProfileNurbs = $.geometry.periodicControlNurbs("curve27ProfileNurbs", {
    controls: [{
      key: "control1",
      position: point46ProfileSplineControl.point,
      weight: 1,
    }, {
      key: "control2",
      position: point47ProfileSplineControl.point,
      weight: 1,
    }, {
      key: "control3",
      position: point48ProfileSplineControl.point,
      weight: 1,
    }, {
      key: "control4",
      position: point49ProfileSplineControl.point,
      weight: 1,
    }, {
      key: "control5",
      position: point50ProfileSplineControl.point,
      weight: 1,
    }],
    degree: 2,
    gauge: "control1",
    label: "Profile NURBS",
    role: "profile",
  });
  const curve28ProfileVerticalSplitter = $.geometry.segment("curve28ProfileVerticalSplitter", {
    start: point51ProfileSplitterStart.point,
    end: point52ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve29ProfileVerticalSplitter = $.geometry.segment("curve29ProfileVerticalSplitter", {
    start: point53ProfileSplitterStart.point,
    end: point54ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve30ProfileVerticalSplitter = $.geometry.segment("curve30ProfileVerticalSplitter", {
    start: point55ProfileSplitterStart.point,
    end: point56ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve31ProfileVerticalSplitter = $.geometry.segment("curve31ProfileVerticalSplitter", {
    start: point57ProfileSplitterStart.point,
    end: point58ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve32ProfileVerticalSplitter = $.geometry.segment("curve32ProfileVerticalSplitter", {
    start: point59ProfileSplitterStart.point,
    end: point60ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve33ProfileVerticalSplitter = $.geometry.segment("curve33ProfileVerticalSplitter", {
    start: point61ProfileSplitterStart.point,
    end: point62ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve34ProfileVerticalSplitter = $.geometry.segment("curve34ProfileVerticalSplitter", {
    start: point63ProfileSplitterStart.point,
    end: point64ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve35ProfileVerticalSplitter = $.geometry.segment("curve35ProfileVerticalSplitter", {
    start: point65ProfileSplitterStart.point,
    end: point66ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve36ProfileVerticalSplitter = $.geometry.segment("curve36ProfileVerticalSplitter", {
    start: point67ProfileSplitterStart.point,
    end: point68ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve37ProfileVerticalSplitter = $.geometry.segment("curve37ProfileVerticalSplitter", {
    start: point69ProfileSplitterStart.point,
    end: point70ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve38ProfileVerticalSplitter = $.geometry.segment("curve38ProfileVerticalSplitter", {
    start: point71ProfileSplitterStart.point,
    end: point72ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve39ProfileVerticalSplitter = $.geometry.segment("curve39ProfileVerticalSplitter", {
    start: point73ProfileSplitterStart.point,
    end: point74ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve40ProfileVerticalSplitter = $.geometry.segment("curve40ProfileVerticalSplitter", {
    start: point75ProfileSplitterStart.point,
    end: point76ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve41ProfileVerticalSplitter = $.geometry.segment("curve41ProfileVerticalSplitter", {
    start: point77ProfileSplitterStart.point,
    end: point78ProfileSplitterEnd.point,
    branchDirection: [0, 1],
    label: "Profile vertical splitter",
    role: "profile",
  });
  const curve42ProfileHyperbolaHorizontalSplitter = $.geometry.segment("curve42ProfileHyperbolaHorizontalSplitter", {
    start: point79ProfileHyperbolaSplitterStart.point,
    end: point80ProfileHyperbolaSplitterEnd.point,
    branchDirection: [1, 0],
    label: "Profile hyperbola horizontal splitter",
    role: "profile",
  });
  const constraint1ProfileCircularArcFirstJoin = $.constraint.pointOnCurve("constraint1ProfileCircularArcFirstJoin", {
    point: point21ProfileCircularArcFirstEndpoint.point,
    curve: curve14ProfileCircularArc.span,
    contact: {
      parameter: 0,
      winding: 0,
      neighborhood: {
        kind: "start",
      },
      orientation: "none",
    },
    label: "Profile circular arc first join",
  });
  const constraint2ProfileCircularArcSecondJoin = $.constraint.pointOnCurve("constraint2ProfileCircularArcSecondJoin", {
    point: point22ProfileCircularArcSecondEndpoint.point,
    curve: curve14ProfileCircularArc.span,
    contact: {
      parameter: 1,
      winding: 0,
      neighborhood: {
        kind: "end",
      },
      orientation: "none",
    },
    label: "Profile circular arc second join",
  });
  const constraint3ProfileEllipticalArcFirstJoin = $.constraint.pointOnCurve("constraint3ProfileEllipticalArcFirstJoin", {
    point: point25ProfileEllipticalArcFirstEndpoint.point,
    curve: curve16ProfileEllipticalArc.span,
    contact: {
      parameter: 0,
      winding: 0,
      neighborhood: {
        kind: "start",
      },
      orientation: "none",
    },
    label: "Profile elliptical arc first join",
  });
  const constraint4ProfileEllipticalArcSecondJoin = $.constraint.pointOnCurve("constraint4ProfileEllipticalArcSecondJoin", {
    point: point26ProfileEllipticalArcSecondEndpoint.point,
    curve: curve16ProfileEllipticalArc.span,
    contact: {
      parameter: 1,
      winding: 0,
      neighborhood: {
        kind: "end",
      },
      orientation: "none",
    },
    label: "Profile elliptical arc second join",
  });
  const constraint5ProfileParabolaFirstJoin = $.constraint.pointOnCurve("constraint5ProfileParabolaFirstJoin", {
    point: point29ProfileParabolaFirstEndpoint.point,
    curve: curve18ProfileParabola.span,
    contact: {
      parameter: 0,
      winding: 0,
      neighborhood: {
        kind: "start",
      },
      orientation: "none",
    },
    label: "Profile parabola first join",
  });
  const constraint6ProfileParabolaSecondJoin = $.constraint.pointOnCurve("constraint6ProfileParabolaSecondJoin", {
    point: point30ProfileParabolaSecondEndpoint.point,
    curve: curve18ProfileParabola.span,
    contact: {
      parameter: 1,
      winding: 0,
      neighborhood: {
        kind: "end",
      },
      orientation: "none",
    },
    label: "Profile parabola second join",
  });
  const constraint7ProfileHyperbolaFirstJoin = $.constraint.pointOnCurve("constraint7ProfileHyperbolaFirstJoin", {
    point: point33ProfileHyperbolaFirstEndpoint.point,
    curve: curve20ProfileHyperbola.span,
    contact: {
      parameter: 0,
      winding: 0,
      neighborhood: {
        kind: "start",
      },
      orientation: "none",
    },
    label: "Profile hyperbola first join",
  });
  const constraint8ProfileHyperbolaSecondJoin = $.constraint.pointOnCurve("constraint8ProfileHyperbolaSecondJoin", {
    point: point34ProfileHyperbolaSecondEndpoint.point,
    curve: curve20ProfileHyperbola.span,
    contact: {
      parameter: 1,
      winding: 0,
      neighborhood: {
        kind: "end",
      },
      orientation: "none",
    },
    label: "Profile hyperbola second join",
  });
  $.group("Points", [point1ProfilePolylinePoint, point2ProfilePolylinePoint, point3ProfilePolylinePoint, point4ProfileLinePoint, point5ProfileLinePoint, point6ProfileLinePoint, point7ProfileLinePoint, point8ProfileCircleCenter, point9ProfileEllipseCenter, point10ProfileEllipseAxis, point11ProfileQuadraticControl, point12ProfileQuadraticControl, point13ProfileQuadraticControl, point14ProfileCubicControl, point15ProfileCubicControl, point16ProfileCubicControl, point17ProfileCubicControl, point18ProfileRationalStart, point19ProfileRationalEnd, point20ProfileCircularArcCenter, point21ProfileCircularArcFirstEndpoint, point22ProfileCircularArcSecondEndpoint, point23ProfileEllipticalArcCenter, point24ProfileEllipticalArcAxis, point25ProfileEllipticalArcFirstEndpoint, point26ProfileEllipticalArcSecondEndpoint, point27ProfileParabolaVertex, point28ProfileParabolaFocus, point29ProfileParabolaFirstEndpoint, point30ProfileParabolaSecondEndpoint, point31ProfileHyperbolaCenter, point32ProfileHyperbolaAxis, point33ProfileHyperbolaFirstEndpoint, point34ProfileHyperbolaSecondEndpoint, point35ProfileSplineControl, point36ProfileSplineControl, point37ProfileSplineControl, point38ProfileSplineControl, point39ProfileSplineControl, point40ProfileSplineControl, point41ProfileSplineControl, point42ProfileSplineControl, point43ProfileSplineControl, point44ProfileSplineControl, point45ProfileSplineControl, point46ProfileSplineControl, point47ProfileSplineControl, point48ProfileSplineControl, point49ProfileSplineControl, point50ProfileSplineControl, point51ProfileSplitterStart, point52ProfileSplitterEnd, point53ProfileSplitterStart, point54ProfileSplitterEnd, point55ProfileSplitterStart, point56ProfileSplitterEnd, point57ProfileSplitterStart, point58ProfileSplitterEnd, point59ProfileSplitterStart, point60ProfileSplitterEnd, point61ProfileSplitterStart, point62ProfileSplitterEnd, point63ProfileSplitterStart, point64ProfileSplitterEnd, point65ProfileSplitterStart, point66ProfileSplitterEnd, point67ProfileSplitterStart, point68ProfileSplitterEnd, point69ProfileSplitterStart, point70ProfileSplitterEnd, point71ProfileSplitterStart, point72ProfileSplitterEnd, point73ProfileSplitterStart, point74ProfileSplitterEnd, point75ProfileSplitterStart, point76ProfileSplitterEnd, point77ProfileSplitterStart, point78ProfileSplitterEnd, point79ProfileHyperbolaSplitterStart, point80ProfileHyperbolaSplitterEnd]);
  $.group("Geometry", [curve1ProfileClosedPolyline, curve2ProfileLineSquareEdge, curve3ProfileLineSquareEdge, curve4ProfileLineSquareEdge, curve5ProfileLineSquareEdge, curve6ProfileCircle, curve7ProfileEllipse, curve8ProfileQuadraticBezier, curve9ProfileQuadraticClosure, curve10ProfileCubicBezier, curve11ProfileCubicClosure, curve12ProfileRationalQuadraticConic, curve13ProfileRationalClosure, curve14ProfileCircularArc, curve15ProfileCircularArcClosure, curve16ProfileEllipticalArc, curve17ProfileEllipticalArcClosure, curve18ProfileParabola, curve19ProfileParabolaClosure, curve20ProfileHyperbola, curve21ProfileHyperbolaClosure, curve22ProfileBSpline, curve23ProfileSplineClosure, curve24ProfileBSpline, curve25ProfileNurbs, curve26ProfileSplineClosure, curve27ProfileNurbs, curve28ProfileVerticalSplitter, curve29ProfileVerticalSplitter, curve30ProfileVerticalSplitter, curve31ProfileVerticalSplitter, curve32ProfileVerticalSplitter, curve33ProfileVerticalSplitter, curve34ProfileVerticalSplitter, curve35ProfileVerticalSplitter, curve36ProfileVerticalSplitter, curve37ProfileVerticalSplitter, curve38ProfileVerticalSplitter, curve39ProfileVerticalSplitter, curve40ProfileVerticalSplitter, curve41ProfileVerticalSplitter, curve42ProfileHyperbolaHorizontalSplitter]);
  $.group("Constraints", [constraint1ProfileCircularArcFirstJoin, constraint2ProfileCircularArcSecondJoin, constraint3ProfileEllipticalArcFirstJoin, constraint4ProfileEllipticalArcSecondJoin, constraint5ProfileParabolaFirstJoin, constraint6ProfileParabolaSecondJoin, constraint7ProfileHyperbolaFirstJoin, constraint8ProfileHyperbolaSecondJoin]);
  return {};
});
