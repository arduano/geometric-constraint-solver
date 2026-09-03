"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1PeaucellierFixedOriginO = $.geometry.sketchPoint("point1PeaucellierFixedOriginO", {
    point: [0, 0],
    label: "Peaucellier fixed origin O",
  });
  const point2PeaucellierInputCenterS = $.geometry.sketchPoint("point2PeaucellierInputCenterS", {
    point: [4, 0],
    label: "Peaucellier input center S",
  });
  const point3PeaucellierCircularInputP = $.geometry.sketchPoint("point3PeaucellierCircularInputP", {
    point: [4, 4],
    label: "Peaucellier circular input P",
  });
  const point4PeaucellierStraightLineOutputQ = $.geometry.sketchPoint("point4PeaucellierStraightLineOutputQ", {
    point: [2, 2],
    label: "Peaucellier straight-line output Q",
  });
  const point5PeaucellierShoulderB = $.geometry.sketchPoint("point5PeaucellierShoulderB", {
    point: [1.1291713066130293, 4.87082869338697],
    label: "Peaucellier shoulder B",
  });
  const point6PeaucellierShoulderD = $.geometry.sketchPoint("point6PeaucellierShoulderD", {
    point: [4.87082869338697, 1.1291713066130293],
    label: "Peaucellier shoulder D",
  });
  const curve1PeaucellierLongBarOb = $.geometry.segment("curve1PeaucellierLongBarOb", {
    start: point1PeaucellierFixedOriginO.point,
    end: point5PeaucellierShoulderB.point,
    branchDirection: [0.22583426132260587, 0.9741657386773941],
    label: "Peaucellier long bar OB",
    role: "profile",
  });
  const curve2PeaucellierLongBarOd = $.geometry.segment("curve2PeaucellierLongBarOd", {
    start: point1PeaucellierFixedOriginO.point,
    end: point6PeaucellierShoulderD.point,
    branchDirection: [0.9741657386773941, 0.22583426132260587],
    label: "Peaucellier long bar OD",
    role: "profile",
  });
  const curve3PeaucellierRhombusBarBp = $.geometry.segment("curve3PeaucellierRhombusBarBp", {
    start: point5PeaucellierShoulderB.point,
    end: point3PeaucellierCircularInputP.point,
    branchDirection: [0.9569428977956568, -0.2902762311289902],
    label: "Peaucellier rhombus bar BP",
    role: "profile",
  });
  const curve4PeaucellierRhombusBarPd = $.geometry.segment("curve4PeaucellierRhombusBarPd", {
    start: point3PeaucellierCircularInputP.point,
    end: point6PeaucellierShoulderD.point,
    branchDirection: [0.2902762311289902, -0.9569428977956568],
    label: "Peaucellier rhombus bar PD",
    role: "profile",
  });
  const curve5PeaucellierRhombusBarDq = $.geometry.segment("curve5PeaucellierRhombusBarDq", {
    start: point6PeaucellierShoulderD.point,
    end: point4PeaucellierStraightLineOutputQ.point,
    branchDirection: [-0.9569428977956568, 0.2902762311289902],
    label: "Peaucellier rhombus bar DQ",
    role: "profile",
  });
  const curve6PeaucellierRhombusBarQb = $.geometry.segment("curve6PeaucellierRhombusBarQb", {
    start: point4PeaucellierStraightLineOutputQ.point,
    end: point5PeaucellierShoulderB.point,
    branchDirection: [-0.2902762311289902, 0.9569428977956568],
    label: "Peaucellier rhombus bar QB",
    role: "profile",
  });
  const curve7PeaucellierCircularDriverSp = $.geometry.segment("curve7PeaucellierCircularDriverSp", {
    start: point2PeaucellierInputCenterS.point,
    end: point3PeaucellierCircularInputP.point,
    branchDirection: [0, 1],
    label: "Peaucellier circular driver SP",
    role: "profile",
  });
  const constraint1PeaucellierOriginFixed = $.constraint.fixedPoint("constraint1PeaucellierOriginFixed", {
    point: point1PeaucellierFixedOriginO.point,
    target: [0, 0],
    label: "Peaucellier origin fixed",
  });
  const constraint2PeaucellierInputCenterFixed = $.constraint.fixedPoint("constraint2PeaucellierInputCenterFixed", {
    point: point2PeaucellierInputCenterS.point,
    target: [4, 0],
    label: "Peaucellier input center fixed",
  });
  const dimension1PeaucellierLongRadius5 = $.dimension.curveLength("dimension1PeaucellierLongRadius5", {
    curve: curve1PeaucellierLongBarOb.span,
    value: mm(5),
    label: "Peaucellier long radius 5",
    mode: "driving",
  });
  const constraint3PeaucellierLongBarsEqual = $.constraint.equalLength("constraint3PeaucellierLongBarsEqual", {
    first: curve1PeaucellierLongBarOb.span,
    second: curve2PeaucellierLongBarOd.span,
    label: "Peaucellier long bars equal",
  });
  const dimension2PeaucellierRhombusSide3 = $.dimension.curveLength("dimension2PeaucellierRhombusSide3", {
    curve: curve3PeaucellierRhombusBarBp.span,
    value: mm(3),
    label: "Peaucellier rhombus side 3",
    mode: "driving",
  });
  const constraint4PeaucellierRhombusSide2Equal = $.constraint.equalLength("constraint4PeaucellierRhombusSide2Equal", {
    first: curve3PeaucellierRhombusBarBp.span,
    second: curve4PeaucellierRhombusBarPd.span,
    label: "Peaucellier rhombus side 2 equal",
  });
  const constraint5PeaucellierRhombusSide3Equal = $.constraint.equalLength("constraint5PeaucellierRhombusSide3Equal", {
    first: curve3PeaucellierRhombusBarBp.span,
    second: curve5PeaucellierRhombusBarDq.span,
    label: "Peaucellier rhombus side 3 equal",
  });
  const constraint6PeaucellierRhombusSide4Equal = $.constraint.equalLength("constraint6PeaucellierRhombusSide4Equal", {
    first: curve3PeaucellierRhombusBarBp.span,
    second: curve6PeaucellierRhombusBarQb.span,
    label: "Peaucellier rhombus side 4 equal",
  });
  const dimension3PeaucellierInputCircleRadius4 = $.dimension.curveLength("dimension3PeaucellierInputCircleRadius4", {
    curve: curve7PeaucellierCircularDriverSp.span,
    value: mm(4),
    label: "Peaucellier input circle radius 4",
    mode: "driving",
  });
  $.group("Points", [point1PeaucellierFixedOriginO, point2PeaucellierInputCenterS, point3PeaucellierCircularInputP, point4PeaucellierStraightLineOutputQ, point5PeaucellierShoulderB, point6PeaucellierShoulderD]);
  $.group("Geometry", [curve1PeaucellierLongBarOb, curve2PeaucellierLongBarOd, curve3PeaucellierRhombusBarBp, curve4PeaucellierRhombusBarPd, curve5PeaucellierRhombusBarDq, curve6PeaucellierRhombusBarQb, curve7PeaucellierCircularDriverSp]);
  $.group("Constraints", [constraint1PeaucellierOriginFixed, constraint2PeaucellierInputCenterFixed, constraint3PeaucellierLongBarsEqual, constraint4PeaucellierRhombusSide2Equal, constraint5PeaucellierRhombusSide3Equal, constraint6PeaucellierRhombusSide4Equal]);
  $.group("Dimensions", [dimension1PeaucellierLongRadius5, dimension2PeaucellierRhombusSide3, dimension3PeaucellierInputCircleRadius4]);
  return {};
});
