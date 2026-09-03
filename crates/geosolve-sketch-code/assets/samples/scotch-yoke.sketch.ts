"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1YokeCrankCenterO = $.geometry.sketchPoint("point1YokeCrankCenterO", {
    point: [0, 0],
    label: "Yoke crank center O",
  });
  const point2YokeCrankPinP = $.geometry.sketchPoint("point2YokeCrankPinP", {
    point: [3, 4],
    label: "Yoke crank pin P",
  });
  const point3YokeHorizontalSliderS = $.geometry.sketchPoint("point3YokeHorizontalSliderS", {
    point: [3, -6],
    label: "Yoke horizontal slider S",
  });
  const curve1YokeCrankOp = $.geometry.segment("curve1YokeCrankOp", {
    start: point1YokeCrankCenterO.point,
    end: point2YokeCrankPinP.point,
    branchDirection: [0.6, 0.8],
    label: "Yoke crank OP",
    role: "profile",
  });
  const curve2YokeVerticalSlotSp = $.geometry.segment("curve2YokeVerticalSlotSp", {
    start: point3YokeHorizontalSliderS.point,
    end: point2YokeCrankPinP.point,
    branchDirection: [0, 1],
    label: "Yoke vertical slot SP",
    role: "profile",
  });
  const constraint1YokeCrankCenterFixed = $.constraint.fixedPoint("constraint1YokeCrankCenterFixed", {
    point: point1YokeCrankCenterO.point,
    target: [0, 0],
    label: "Yoke crank center fixed",
  });
  const constraint2YokeSliderOnHorizontalGuide = $.constraint.fixedCoordinate("constraint2YokeSliderOnHorizontalGuide", {
    point: point3YokeHorizontalSliderS.point,
    axis: "y",
    target: mm(-6),
    label: "Yoke slider on horizontal guide",
  });
  const constraint3YokeSlotRemainsVertical = $.constraint.vertical("constraint3YokeSlotRemainsVertical", {
    span: curve2YokeVerticalSlotSp.span,
    label: "Yoke slot remains vertical",
  });
  const dimension1YokeCrankRadius5 = $.dimension.curveLength("dimension1YokeCrankRadius5", {
    curve: curve1YokeCrankOp.span,
    value: mm(5),
    label: "Yoke crank radius 5",
    mode: "driving",
  });
  $.group("Points", [point1YokeCrankCenterO, point2YokeCrankPinP, point3YokeHorizontalSliderS]);
  $.group("Geometry", [curve1YokeCrankOp, curve2YokeVerticalSlotSp]);
  $.group("Constraints", [constraint1YokeCrankCenterFixed, constraint2YokeSliderOnHorizontalGuide, constraint3YokeSlotRemainsVertical]);
  $.group("Dimensions", [dimension1YokeCrankRadius5]);
  return {};
});
