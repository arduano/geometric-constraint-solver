"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1FixedSpecimenPoint = $.geometry.sketchPoint("point1FixedSpecimenPoint", {
    point: [-20, 12],
    label: "Fixed specimen point",
  });
  const point2CoincidentFirstPoint = $.geometry.sketchPoint("point2CoincidentFirstPoint", {
    point: [-10, 12],
    label: "Coincident first point",
  });
  const point3CoincidentSecondPoint = $.geometry.sketchPoint("point3CoincidentSecondPoint", {
    point: [-10, 12],
    label: "Coincident second point",
  });
  const point4CoincidentFirstArm = $.geometry.sketchPoint("point4CoincidentFirstArm", {
    point: [-12.5, 10.5],
    label: "Coincident first arm",
  });
  const point5CoincidentSecondArm = $.geometry.sketchPoint("point5CoincidentSecondArm", {
    point: [-7.5, 13.5],
    label: "Coincident second arm",
  });
  const point6HorizontalStart = $.geometry.sketchPoint("point6HorizontalStart", {
    point: [-3, 12],
    label: "Horizontal start",
  });
  const point7HorizontalEnd = $.geometry.sketchPoint("point7HorizontalEnd", {
    point: [3, 12],
    label: "Horizontal end",
  });
  const point8VerticalStart = $.geometry.sketchPoint("point8VerticalStart", {
    point: [10, 9.5],
    label: "Vertical start",
  });
  const point9VerticalEnd = $.geometry.sketchPoint("point9VerticalEnd", {
    point: [10, 14.5],
    label: "Vertical end",
  });
  const point10PointOnCurveStart = $.geometry.sketchPoint("point10PointOnCurveStart", {
    point: [17, 12],
    label: "Point-on-curve start",
  });
  const point11PointOnCurveEnd = $.geometry.sketchPoint("point11PointOnCurveEnd", {
    point: [23, 12],
    label: "Point-on-curve end",
  });
  const point12PointOnCurve = $.geometry.sketchPoint("point12PointOnCurve", {
    point: [20, 12],
    label: "Point on curve",
  });
  const point13ParallelFirstStart = $.geometry.sketchPoint("point13ParallelFirstStart", {
    point: [-23, 3],
    label: "Parallel first start",
  });
  const point14ParallelFirstEnd = $.geometry.sketchPoint("point14ParallelFirstEnd", {
    point: [-17, 3],
    label: "Parallel first end",
  });
  const point15ParallelSecondStart = $.geometry.sketchPoint("point15ParallelSecondStart", {
    point: [-23, 5],
    label: "Parallel second start",
  });
  const point16ParallelSecondEnd = $.geometry.sketchPoint("point16ParallelSecondEnd", {
    point: [-17, 5],
    label: "Parallel second end",
  });
  const point17PerpendicularHorizontalStart = $.geometry.sketchPoint("point17PerpendicularHorizontalStart", {
    point: [-13, 4],
    label: "Perpendicular horizontal start",
  });
  const point18PerpendicularHorizontalEnd = $.geometry.sketchPoint("point18PerpendicularHorizontalEnd", {
    point: [-7, 4],
    label: "Perpendicular horizontal end",
  });
  const point19PerpendicularVerticalStart = $.geometry.sketchPoint("point19PerpendicularVerticalStart", {
    point: [-10, 1.5],
    label: "Perpendicular vertical start",
  });
  const point20PerpendicularVerticalEnd = $.geometry.sketchPoint("point20PerpendicularVerticalEnd", {
    point: [-10, 6.5],
    label: "Perpendicular vertical end",
  });
  const point21ConcentricFirstCenter = $.geometry.sketchPoint("point21ConcentricFirstCenter", {
    point: [0, 4],
    label: "Concentric first center",
  });
  const point22ConcentricSecondCenter = $.geometry.sketchPoint("point22ConcentricSecondCenter", {
    point: [0, 4],
    label: "Concentric second center",
  });
  const point23CollinearFirstStart = $.geometry.sketchPoint("point23CollinearFirstStart", {
    point: [6.5, 4],
    label: "Collinear first start",
  });
  const point24CollinearFirstEnd = $.geometry.sketchPoint("point24CollinearFirstEnd", {
    point: [9.5, 4],
    label: "Collinear first end",
  });
  const point25CollinearSecondStart = $.geometry.sketchPoint("point25CollinearSecondStart", {
    point: [10.5, 4],
    label: "Collinear second start",
  });
  const point26CollinearSecondEnd = $.geometry.sketchPoint("point26CollinearSecondEnd", {
    point: [13.5, 4],
    label: "Collinear second end",
  });
  const point27EqualLengthFirstStart = $.geometry.sketchPoint("point27EqualLengthFirstStart", {
    point: [17, 2.7],
    label: "Equal-length first start",
  });
  const point28EqualLengthFirstEnd = $.geometry.sketchPoint("point28EqualLengthFirstEnd", {
    point: [20, 2.7],
    label: "Equal-length first end",
  });
  const point29EqualLengthSecondStart = $.geometry.sketchPoint("point29EqualLengthSecondStart", {
    point: [20, 5.3],
    label: "Equal-length second start",
  });
  const point30EqualLengthSecondEnd = $.geometry.sketchPoint("point30EqualLengthSecondEnd", {
    point: [23, 5.3],
    label: "Equal-length second end",
  });
  const point31EqualRadiusFirstCenter = $.geometry.sketchPoint("point31EqualRadiusFirstCenter", {
    point: [-22, -4],
    label: "Equal-radius first center",
  });
  const point32EqualRadiusSecondCenter = $.geometry.sketchPoint("point32EqualRadiusSecondCenter", {
    point: [-18, -4],
    label: "Equal-radius second center",
  });
  const point33MidpointStart = $.geometry.sketchPoint("point33MidpointStart", {
    point: [-13, -4],
    label: "Midpoint start",
  });
  const point34MidpointEnd = $.geometry.sketchPoint("point34MidpointEnd", {
    point: [-7, -4],
    label: "Midpoint end",
  });
  const point35MidpointPoint = $.geometry.sketchPoint("point35MidpointPoint", {
    point: [-10, -4],
    label: "Midpoint point",
  });
  const point36SymmetryAxisStart = $.geometry.sketchPoint("point36SymmetryAxisStart", {
    point: [-3, -4],
    label: "Symmetry axis start",
  });
  const point37SymmetryAxisEnd = $.geometry.sketchPoint("point37SymmetryAxisEnd", {
    point: [3, -4],
    label: "Symmetry axis end",
  });
  const point38SymmetricFirstPoint = $.geometry.sketchPoint("point38SymmetricFirstPoint", {
    point: [0, -2],
    label: "Symmetric first point",
  });
  const point39SymmetricSecondPoint = $.geometry.sketchPoint("point39SymmetricSecondPoint", {
    point: [0, -6],
    label: "Symmetric second point",
  });
  const point40ContactHorizontalStart = $.geometry.sketchPoint("point40ContactHorizontalStart", {
    point: [7, -4],
    label: "Contact horizontal start",
  });
  const point41ContactHorizontalEnd = $.geometry.sketchPoint("point41ContactHorizontalEnd", {
    point: [13, -4],
    label: "Contact horizontal end",
  });
  const point42ContactVerticalStart = $.geometry.sketchPoint("point42ContactVerticalStart", {
    point: [10, -6.5],
    label: "Contact vertical start",
  });
  const point43ContactVerticalEnd = $.geometry.sketchPoint("point43ContactVerticalEnd", {
    point: [10, -1.5],
    label: "Contact vertical end",
  });
  const point44TangencyLineStart = $.geometry.sketchPoint("point44TangencyLineStart", {
    point: [17, -4],
    label: "Tangency line start",
  });
  const point45TangencyLineEnd = $.geometry.sketchPoint("point45TangencyLineEnd", {
    point: [23, -4],
    label: "Tangency line end",
  });
  const point46TangencyCircleCenter = $.geometry.sketchPoint("point46TangencyCircleCenter", {
    point: [20, -2.5],
    label: "Tangency circle center",
  });
  const point47DirectionCircleCenter = $.geometry.sketchPoint("point47DirectionCircleCenter", {
    point: [-20, -13.5],
    label: "Direction circle center",
  });
  const point48TangentDirectionLineStart = $.geometry.sketchPoint("point48TangentDirectionLineStart", {
    point: [-23, -12],
    label: "Tangent-direction line start",
  });
  const point49TangentDirectionLineEnd = $.geometry.sketchPoint("point49TangentDirectionLineEnd", {
    point: [-17, -12],
    label: "Tangent-direction line end",
  });
  const point50NormalCircleCenter = $.geometry.sketchPoint("point50NormalCircleCenter", {
    point: [-10, -12],
    label: "Normal circle center",
  });
  const point51NormalDirectionLineStart = $.geometry.sketchPoint("point51NormalDirectionLineStart", {
    point: [-13, -12],
    label: "Normal-direction line start",
  });
  const point52NormalDirectionLineEnd = $.geometry.sketchPoint("point52NormalDirectionLineEnd", {
    point: [-7, -12],
    label: "Normal-direction line end",
  });
  const point53EqualCurvatureFirstCenter = $.geometry.sketchPoint("point53EqualCurvatureFirstCenter", {
    point: [-2, -12],
    label: "Equal-curvature first center",
  });
  const point54EqualCurvatureSecondCenter = $.geometry.sketchPoint("point54EqualCurvatureSecondCenter", {
    point: [2, -12],
    label: "Equal-curvature second center",
  });
  const point55ContinuityFirstStart = $.geometry.sketchPoint("point55ContinuityFirstStart", {
    point: [7, -12],
    label: "Continuity first start",
  });
  const point56ContinuitySeam = $.geometry.sketchPoint("point56ContinuitySeam", {
    point: [10, -12],
    label: "Continuity seam",
  });
  const point57ContinuitySecondEnd = $.geometry.sketchPoint("point57ContinuitySecondEnd", {
    point: [13, -12],
    label: "Continuity second end",
  });
  const point58FilletHorizontalStart = $.geometry.sketchPoint("point58FilletHorizontalStart", {
    point: [17, -12],
    label: "Fillet horizontal start",
  });
  const point59FilletHorizontalEnd = $.geometry.sketchPoint("point59FilletHorizontalEnd", {
    point: [23, -12],
    label: "Fillet horizontal end",
  });
  const point60FilletVerticalStart = $.geometry.sketchPoint("point60FilletVerticalStart", {
    point: [20, -14.5],
    label: "Fillet vertical start",
  });
  const point61FilletVerticalEnd = $.geometry.sketchPoint("point61FilletVerticalEnd", {
    point: [20, -9.5],
    label: "Fillet vertical end",
  });
  const point62FilletCenter = $.geometry.sketchPoint("point62FilletCenter", {
    point: [19, -11],
    label: "Fillet.center",
  });
  const curve1CoincidentFirstSupport = $.geometry.segment("curve1CoincidentFirstSupport", {
    start: point4CoincidentFirstArm.point,
    end: point2CoincidentFirstPoint.point,
    branchDirection: [0.8574929257125441, 0.5144957554275265],
    label: "Coincident first support",
    role: "profile",
  });
  const curve2CoincidentSecondSupport = $.geometry.segment("curve2CoincidentSecondSupport", {
    start: point3CoincidentSecondPoint.point,
    end: point5CoincidentSecondArm.point,
    branchDirection: [0.8574929257125441, 0.5144957554275265],
    label: "Coincident second support",
    role: "profile",
  });
  const curve3HorizontalSpecimen = $.geometry.segment("curve3HorizontalSpecimen", {
    start: point6HorizontalStart.point,
    end: point7HorizontalEnd.point,
    branchDirection: [1, 0],
    label: "Horizontal specimen",
    role: "profile",
  });
  const curve4VerticalSpecimen = $.geometry.segment("curve4VerticalSpecimen", {
    start: point8VerticalStart.point,
    end: point9VerticalEnd.point,
    branchDirection: [0, 1],
    label: "Vertical specimen",
    role: "profile",
  });
  const curve5PointOnCurveSupport = $.geometry.segment("curve5PointOnCurveSupport", {
    start: point10PointOnCurveStart.point,
    end: point11PointOnCurveEnd.point,
    branchDirection: [1, 0],
    label: "Point-on-curve support",
    role: "profile",
  });
  const curve6ParallelFirstSupport = $.geometry.segment("curve6ParallelFirstSupport", {
    start: point13ParallelFirstStart.point,
    end: point14ParallelFirstEnd.point,
    branchDirection: [1, 0],
    label: "Parallel first support",
    role: "profile",
  });
  const curve7ParallelSecondSupport = $.geometry.segment("curve7ParallelSecondSupport", {
    start: point15ParallelSecondStart.point,
    end: point16ParallelSecondEnd.point,
    branchDirection: [1, 0],
    label: "Parallel second support",
    role: "profile",
  });
  const curve8PerpendicularHorizontalSupport = $.geometry.segment("curve8PerpendicularHorizontalSupport", {
    start: point17PerpendicularHorizontalStart.point,
    end: point18PerpendicularHorizontalEnd.point,
    branchDirection: [1, 0],
    label: "Perpendicular horizontal support",
    role: "profile",
  });
  const curve9PerpendicularVerticalSupport = $.geometry.segment("curve9PerpendicularVerticalSupport", {
    start: point19PerpendicularVerticalStart.point,
    end: point20PerpendicularVerticalEnd.point,
    branchDirection: [0, 1],
    label: "Perpendicular vertical support",
    role: "profile",
  });
  const curve10ConcentricOuterCircle = $.geometry.centerRadiusCircle("curve10ConcentricOuterCircle", {
    center: point21ConcentricFirstCenter.point,
    radius: mm(2.3),
    label: "Concentric outer circle",
    role: "profile",
  });
  const curve11ConcentricInnerCircle = $.geometry.centerRadiusCircle("curve11ConcentricInnerCircle", {
    center: point22ConcentricSecondCenter.point,
    radius: mm(1.2),
    label: "Concentric inner circle",
    role: "profile",
  });
  const curve12CollinearFirstSupport = $.geometry.segment("curve12CollinearFirstSupport", {
    start: point23CollinearFirstStart.point,
    end: point24CollinearFirstEnd.point,
    branchDirection: [1, 0],
    label: "Collinear first support",
    role: "profile",
  });
  const curve13CollinearSecondSupport = $.geometry.segment("curve13CollinearSecondSupport", {
    start: point25CollinearSecondStart.point,
    end: point26CollinearSecondEnd.point,
    branchDirection: [1, 0],
    label: "Collinear second support",
    role: "profile",
  });
  const curve14EqualLengthFirstSupport = $.geometry.segment("curve14EqualLengthFirstSupport", {
    start: point27EqualLengthFirstStart.point,
    end: point28EqualLengthFirstEnd.point,
    branchDirection: [1, 0],
    label: "Equal-length first support",
    role: "profile",
  });
  const curve15EqualLengthSecondSupport = $.geometry.segment("curve15EqualLengthSecondSupport", {
    start: point29EqualLengthSecondStart.point,
    end: point30EqualLengthSecondEnd.point,
    branchDirection: [1, 0],
    label: "Equal-length second support",
    role: "profile",
  });
  const curve16EqualRadiusFirstCircle = $.geometry.centerRadiusCircle("curve16EqualRadiusFirstCircle", {
    center: point31EqualRadiusFirstCenter.point,
    radius: mm(1.4),
    label: "Equal-radius first circle",
    role: "profile",
  });
  const curve17EqualRadiusSecondCircle = $.geometry.centerRadiusCircle("curve17EqualRadiusSecondCircle", {
    center: point32EqualRadiusSecondCenter.point,
    radius: mm(1.4),
    label: "Equal-radius second circle",
    role: "profile",
  });
  const curve18MidpointSupport = $.geometry.segment("curve18MidpointSupport", {
    start: point33MidpointStart.point,
    end: point34MidpointEnd.point,
    branchDirection: [1, 0],
    label: "Midpoint support",
    role: "profile",
  });
  const curve19SymmetryAxis = $.geometry.segment("curve19SymmetryAxis", {
    start: point36SymmetryAxisStart.point,
    end: point37SymmetryAxisEnd.point,
    branchDirection: [1, 0],
    label: "Symmetry axis",
    role: "profile",
  });
  const curve20ContactHorizontalSupport = $.geometry.segment("curve20ContactHorizontalSupport", {
    start: point40ContactHorizontalStart.point,
    end: point41ContactHorizontalEnd.point,
    branchDirection: [1, 0],
    label: "Contact horizontal support",
    role: "profile",
  });
  const curve21ContactVerticalSupport = $.geometry.segment("curve21ContactVerticalSupport", {
    start: point42ContactVerticalStart.point,
    end: point43ContactVerticalEnd.point,
    branchDirection: [0, 1],
    label: "Contact vertical support",
    role: "profile",
  });
  const curve22TangencyLine = $.geometry.segment("curve22TangencyLine", {
    start: point44TangencyLineStart.point,
    end: point45TangencyLineEnd.point,
    branchDirection: [1, 0],
    label: "Tangency line",
    role: "profile",
  });
  const curve23TangencyCircle = $.geometry.centerRadiusCircle("curve23TangencyCircle", {
    center: point46TangencyCircleCenter.point,
    radius: mm(1.5),
    label: "Tangency circle",
    role: "profile",
  });
  const curve24DirectionCircle = $.geometry.centerRadiusCircle("curve24DirectionCircle", {
    center: point47DirectionCircleCenter.point,
    radius: mm(1.5),
    label: "Direction circle",
    role: "profile",
  });
  const curve25TangentDirectionLine = $.geometry.segment("curve25TangentDirectionLine", {
    start: point48TangentDirectionLineStart.point,
    end: point49TangentDirectionLineEnd.point,
    branchDirection: [1, 0],
    label: "Tangent-direction line",
    role: "profile",
  });
  const curve26NormalCircle = $.geometry.centerRadiusCircle("curve26NormalCircle", {
    center: point50NormalCircleCenter.point,
    radius: mm(1.5),
    label: "Normal circle",
    role: "profile",
  });
  const curve27NormalDirectionLine = $.geometry.segment("curve27NormalDirectionLine", {
    start: point51NormalDirectionLineStart.point,
    end: point52NormalDirectionLineEnd.point,
    branchDirection: [1, 0],
    label: "Normal-direction line",
    role: "profile",
  });
  const curve28EqualCurvatureFirstCircle = $.geometry.centerRadiusCircle("curve28EqualCurvatureFirstCircle", {
    center: point53EqualCurvatureFirstCenter.point,
    radius: mm(1.4),
    label: "Equal-curvature first circle",
    role: "profile",
  });
  const curve29EqualCurvatureSecondCircle = $.geometry.centerRadiusCircle("curve29EqualCurvatureSecondCircle", {
    center: point54EqualCurvatureSecondCenter.point,
    radius: mm(1.4),
    label: "Equal-curvature second circle",
    role: "profile",
  });
  const curve30ContinuityIncomingSupport = $.geometry.segment("curve30ContinuityIncomingSupport", {
    start: point55ContinuityFirstStart.point,
    end: point56ContinuitySeam.point,
    branchDirection: [1, 0],
    label: "Continuity incoming support",
    role: "profile",
  });
  const curve31ContinuityOutgoingSupport = $.geometry.segment("curve31ContinuityOutgoingSupport", {
    start: point56ContinuitySeam.point,
    end: point57ContinuitySecondEnd.point,
    branchDirection: [1, 0],
    label: "Continuity outgoing support",
    role: "profile",
  });
  const curve32FilletHorizontalParent = $.geometry.segment("curve32FilletHorizontalParent", {
    start: point58FilletHorizontalStart.point,
    end: point59FilletHorizontalEnd.point,
    branchDirection: [1, 0],
    label: "Fillet horizontal parent",
    role: "profile",
  });
  const curve33FilletVerticalParent = $.geometry.segment("curve33FilletVerticalParent", {
    start: point60FilletVerticalStart.point,
    end: point61FilletVerticalEnd.point,
    branchDirection: [0, 1],
    label: "Fillet vertical parent",
    role: "profile",
  });
  const curve34FilletArc = $.geometry.centerArc("curve34FilletArc", {
    center: point62FilletCenter.point,
    start: [19, -12],
    end: [20, -11],
    sweep: "counterClockwise",
    label: "Fillet.arc",
    role: "profile",
  });
  const constraint1Fixed = $.constraint.fixedPoint("constraint1Fixed", {
    point: point1FixedSpecimenPoint.point,
    target: [-20, 12],
    label: "Fixed",
  });
  const constraint2Coincident = $.constraint.coincident("constraint2Coincident", {
    first: point2CoincidentFirstPoint.point,
    second: point3CoincidentSecondPoint.point,
    label: "Coincident",
  });
  const constraint3Horizontal = $.constraint.horizontal("constraint3Horizontal", {
    span: curve3HorizontalSpecimen.span,
    label: "Horizontal",
  });
  const constraint4Vertical = $.constraint.vertical("constraint4Vertical", {
    span: curve4VerticalSpecimen.span,
    label: "Vertical",
  });
  const constraint5PointOnCurve = $.constraint.pointOnCurve("constraint5PointOnCurve", {
    point: point12PointOnCurve.point,
    curve: curve5PointOnCurveSupport.span,
    contact: {
      parameter: 0.5,
      winding: 0,
      neighborhood: {
        kind: "interior",
      },
      orientation: "none",
    },
    label: "Point on curve",
  });
  const constraint6Parallel = $.constraint.parallel("constraint6Parallel", {
    first: curve6ParallelFirstSupport.span,
    second: curve7ParallelSecondSupport.span,
    label: "Parallel",
  });
  const constraint7Perpendicular = $.constraint.perpendicular("constraint7Perpendicular", {
    first: curve8PerpendicularHorizontalSupport.span,
    second: curve9PerpendicularVerticalSupport.span,
    label: "Perpendicular",
  });
  const constraint8Concentric = $.constraint.concentric("constraint8Concentric", {
    first: curve10ConcentricOuterCircle.curve,
    second: curve11ConcentricInnerCircle.curve,
    label: "Concentric",
  });
  const constraint9Collinear = $.constraint.collinear("constraint9Collinear", {
    first: curve12CollinearFirstSupport.span,
    second: curve13CollinearSecondSupport.span,
    firstDirection: "forward",
    secondDirection: "forward",
    label: "Collinear",
  });
  const constraint10EqualLength = $.constraint.equalLength("constraint10EqualLength", {
    first: curve14EqualLengthFirstSupport.span,
    second: curve15EqualLengthSecondSupport.span,
    label: "Equal length",
  });
  const constraint11EqualRadius = $.constraint.equalRadius("constraint11EqualRadius", {
    first: curve16EqualRadiusFirstCircle.curve,
    second: curve17EqualRadiusSecondCircle.curve,
    label: "Equal radius",
  });
  const constraint12Midpoint = $.constraint.midpoint("constraint12Midpoint", {
    point: point35MidpointPoint.point,
    line: curve18MidpointSupport.span,
    label: "Midpoint",
  });
  const constraint13Symmetry = $.constraint.symmetricAboutLine("constraint13Symmetry", {
    first: point38SymmetricFirstPoint.point,
    second: point39SymmetricSecondPoint.point,
    axis: curve19SymmetryAxis.span,
    label: "Symmetry",
  });
  const constraint14Contact = $.constraint.curveCurveContact("constraint14Contact", {
    first: curve20ContactHorizontalSupport.span,
    second: curve21ContactVerticalSupport.span,
    contacts: {
      first: {
        parameter: 0.5,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "none",
      },
      second: {
        parameter: 0.5,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "none",
      },
    },
    label: "Contact",
  });
  const constraint15Tangency = $.constraint.lineCircleTangency("constraint15Tangency", {
    line: curve22TangencyLine.span,
    circle: curve23TangencyCircle.curve,
    contacts: {
      first: {
        parameter: 0.5,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "aligned",
      },
      second: {
        parameter: 4.71238898038469,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "aligned",
      },
    },
    side: "left",
    label: "Tangency",
  });
  const constraint16Direction = $.constraint.curveDirection("constraint16Direction", {
    first: curve25TangentDirectionLine.span,
    second: curve24DirectionCircle.span,
    contact: {
      parameter: 1.5707963267948966,
      winding: 0,
      neighborhood: {
        kind: "interior",
      },
      orientation: "none",
    },
    relation: "tangent",
    orientation: "opposed",
    label: "Direction",
  });
  const constraint17Normal = $.constraint.curveDirection("constraint17Normal", {
    first: curve27NormalDirectionLine.span,
    second: curve26NormalCircle.span,
    contact: {
      parameter: 0,
      winding: 0,
      neighborhood: {
        kind: "interior",
      },
      orientation: "none",
    },
    relation: "normal",
    side: "right",
    label: "Normal",
  });
  const constraint18EqualCurvature = $.constraint.equalCurvature("constraint18EqualCurvature", {
    first: curve28EqualCurvatureFirstCircle.span,
    second: curve29EqualCurvatureSecondCircle.span,
    contacts: {
      first: {
        parameter: 0,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "none",
      },
      second: {
        parameter: 0,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "none",
      },
    },
    relation: "signed",
    label: "Equal curvature",
  });
  const constraint19Continuity = $.constraint.endpointContinuity("constraint19Continuity", {
    first: curve30ContinuityIncomingSupport.span,
    second: curve31ContinuityOutgoingSupport.span,
    contacts: {
      first: {
        parameter: 1,
        winding: 0,
        neighborhood: {
          kind: "end",
        },
        orientation: "none",
      },
      second: {
        parameter: 0,
        winding: 0,
        neighborhood: {
          kind: "start",
        },
        orientation: "none",
      },
    },
    continuity: "g1",
    label: "Continuity",
  });
  const constraint20FilletAssociation = $.constraint.lineLineFillet("constraint20FilletAssociation", {
    fillet: curve34FilletArc.curve,
    first: curve32FilletHorizontalParent.span,
    second: curve33FilletVerticalParent.span,
    contacts: {
      first: {
        parameter: 0.3333333333333333,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "none",
      },
      second: {
        parameter: 0.7,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "none",
      },
    },
    firstSide: "left",
    secondSide: "left",
    endpointOrder: "firstThenSecond",
    label: "Fillet.association",
  });
  const dimension1FilletRadiusDimension = $.dimension.radius("dimension1FilletRadiusDimension", {
    curve: curve34FilletArc.curve,
    value: mm(1),
    label: "Fillet.radius_dimension",
    mode: "reference",
  });
  $.group("Points", [point1FixedSpecimenPoint, point2CoincidentFirstPoint, point3CoincidentSecondPoint, point4CoincidentFirstArm, point5CoincidentSecondArm, point6HorizontalStart, point7HorizontalEnd, point8VerticalStart, point9VerticalEnd, point10PointOnCurveStart, point11PointOnCurveEnd, point12PointOnCurve, point13ParallelFirstStart, point14ParallelFirstEnd, point15ParallelSecondStart, point16ParallelSecondEnd, point17PerpendicularHorizontalStart, point18PerpendicularHorizontalEnd, point19PerpendicularVerticalStart, point20PerpendicularVerticalEnd, point21ConcentricFirstCenter, point22ConcentricSecondCenter, point23CollinearFirstStart, point24CollinearFirstEnd, point25CollinearSecondStart, point26CollinearSecondEnd, point27EqualLengthFirstStart, point28EqualLengthFirstEnd, point29EqualLengthSecondStart, point30EqualLengthSecondEnd, point31EqualRadiusFirstCenter, point32EqualRadiusSecondCenter, point33MidpointStart, point34MidpointEnd, point35MidpointPoint, point36SymmetryAxisStart, point37SymmetryAxisEnd, point38SymmetricFirstPoint, point39SymmetricSecondPoint, point40ContactHorizontalStart, point41ContactHorizontalEnd, point42ContactVerticalStart, point43ContactVerticalEnd, point44TangencyLineStart, point45TangencyLineEnd, point46TangencyCircleCenter, point47DirectionCircleCenter, point48TangentDirectionLineStart, point49TangentDirectionLineEnd, point50NormalCircleCenter, point51NormalDirectionLineStart, point52NormalDirectionLineEnd, point53EqualCurvatureFirstCenter, point54EqualCurvatureSecondCenter, point55ContinuityFirstStart, point56ContinuitySeam, point57ContinuitySecondEnd, point58FilletHorizontalStart, point59FilletHorizontalEnd, point60FilletVerticalStart, point61FilletVerticalEnd, point62FilletCenter]);
  $.group("Geometry", [curve1CoincidentFirstSupport, curve2CoincidentSecondSupport, curve3HorizontalSpecimen, curve4VerticalSpecimen, curve5PointOnCurveSupport, curve6ParallelFirstSupport, curve7ParallelSecondSupport, curve8PerpendicularHorizontalSupport, curve9PerpendicularVerticalSupport, curve10ConcentricOuterCircle, curve11ConcentricInnerCircle, curve12CollinearFirstSupport, curve13CollinearSecondSupport, curve14EqualLengthFirstSupport, curve15EqualLengthSecondSupport, curve16EqualRadiusFirstCircle, curve17EqualRadiusSecondCircle, curve18MidpointSupport, curve19SymmetryAxis, curve20ContactHorizontalSupport, curve21ContactVerticalSupport, curve22TangencyLine, curve23TangencyCircle, curve24DirectionCircle, curve25TangentDirectionLine, curve26NormalCircle, curve27NormalDirectionLine, curve28EqualCurvatureFirstCircle, curve29EqualCurvatureSecondCircle, curve30ContinuityIncomingSupport, curve31ContinuityOutgoingSupport, curve32FilletHorizontalParent, curve33FilletVerticalParent, curve34FilletArc]);
  $.group("Constraints", [constraint1Fixed, constraint2Coincident, constraint3Horizontal, constraint4Vertical, constraint5PointOnCurve, constraint6Parallel, constraint7Perpendicular, constraint8Concentric, constraint9Collinear, constraint10EqualLength, constraint11EqualRadius, constraint12Midpoint, constraint13Symmetry, constraint14Contact, constraint15Tangency, constraint16Direction, constraint17Normal, constraint18EqualCurvature, constraint19Continuity, constraint20FilletAssociation]);
  $.group("Dimensions", [dimension1FilletRadiusDimension]);
  return {};
});
