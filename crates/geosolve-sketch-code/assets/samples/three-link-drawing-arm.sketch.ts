"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1DrawingArmFixedAnchorO = $.geometry.sketchPoint("point1DrawingArmFixedAnchorO", {
    point: [0, 0],
    label: "Drawing arm fixed anchor O",
  });
  const point2DrawingArmShoulderA = $.geometry.sketchPoint("point2DrawingArmShoulderA", {
    point: [3, 0],
    label: "Drawing arm shoulder A",
  });
  const point3DrawingArmElbowB = $.geometry.sketchPoint("point3DrawingArmElbowB", {
    point: [5, 2],
    label: "Drawing arm elbow B",
  });
  const point4DrawingArmPenC = $.geometry.sketchPoint("point4DrawingArmPenC", {
    point: [7, 1],
    label: "Drawing arm pen C",
  });
  const curve1DrawingArmLinkOA = $.geometry.segment("curve1DrawingArmLinkOA", {
    start: point1DrawingArmFixedAnchorO.point,
    end: point2DrawingArmShoulderA.point,
    branchDirection: [1, 0],
    label: "Drawing arm link O-A",
    role: "profile",
  });
  const curve2DrawingArmLinkAB = $.geometry.segment("curve2DrawingArmLinkAB", {
    start: point2DrawingArmShoulderA.point,
    end: point3DrawingArmElbowB.point,
    branchDirection: [0.7071067811865476, 0.7071067811865476],
    label: "Drawing arm link A-B",
    role: "profile",
  });
  const curve3DrawingArmLinkBC = $.geometry.segment("curve3DrawingArmLinkBC", {
    start: point3DrawingArmElbowB.point,
    end: point4DrawingArmPenC.point,
    branchDirection: [0.8944271909999159, -0.4472135954999579],
    label: "Drawing arm link B-C",
    role: "profile",
  });
  const constraint1DrawingArmAnchorFixed = $.constraint.fixedPoint("constraint1DrawingArmAnchorFixed", {
    point: point1DrawingArmFixedAnchorO.point,
    target: [0, 0],
    label: "Drawing arm anchor fixed",
  });
  const dimension1DrawingArmLink1Length = $.dimension.curveLength("dimension1DrawingArmLink1Length", {
    curve: curve1DrawingArmLinkOA.span,
    value: mm(3),
    label: "Drawing arm link 1 length",
    mode: "driving",
  });
  const dimension2DrawingArmLink2Length = $.dimension.curveLength("dimension2DrawingArmLink2Length", {
    curve: curve2DrawingArmLinkAB.span,
    value: mm(2.8284271247461903),
    label: "Drawing arm link 2 length",
    mode: "driving",
  });
  const dimension3DrawingArmLink3Length = $.dimension.curveLength("dimension3DrawingArmLink3Length", {
    curve: curve3DrawingArmLinkBC.span,
    value: mm(2.23606797749979),
    label: "Drawing arm link 3 length",
    mode: "driving",
  });
  $.group("Points", [point1DrawingArmFixedAnchorO, point2DrawingArmShoulderA, point3DrawingArmElbowB, point4DrawingArmPenC]);
  $.group("Geometry", [curve1DrawingArmLinkOA, curve2DrawingArmLinkAB, curve3DrawingArmLinkBC]);
  $.group("Constraints", [constraint1DrawingArmAnchorFixed]);
  $.group("Dimensions", [dimension1DrawingArmLink1Length, dimension2DrawingArmLink2Length, dimension3DrawingArmLink3Length]);
  return {};
});
