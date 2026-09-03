"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1CompassPivotO = $.geometry.sketchPoint("point1CompassPivotO", {
    point: [0, 0],
    label: "Compass pivot O",
  });
  const point2CompassFixedBisectorK = $.geometry.sketchPoint("point2CompassFixedBisectorK", {
    point: [4.330127018922193, 2.5],
    label: "Compass fixed bisector K",
  });
  const point3CompassTipA = $.geometry.sketchPoint("point3CompassTipA", {
    point: [4, 0],
    label: "Compass tip A",
  });
  const point4CompassTipB = $.geometry.sketchPoint("point4CompassTipB", {
    point: [2, 3.4641016151377544],
    label: "Compass tip B",
  });
  const curve1CompassSymmetryAxis = $.geometry.segment("curve1CompassSymmetryAxis", {
    start: point1CompassPivotO.point,
    end: point2CompassFixedBisectorK.point,
    branchDirection: [0.8660254037844386, 0.5],
    label: "Compass symmetry axis",
    role: "profile",
  });
  const curve2CompassArmOa = $.geometry.segment("curve2CompassArmOa", {
    start: point1CompassPivotO.point,
    end: point3CompassTipA.point,
    branchDirection: [1, 0],
    label: "Compass arm OA",
    role: "profile",
  });
  const curve3CompassArmOb = $.geometry.segment("curve3CompassArmOb", {
    start: point1CompassPivotO.point,
    end: point4CompassTipB.point,
    branchDirection: [0.5, 0.8660254037844386],
    label: "Compass arm OB",
    role: "profile",
  });
  const curve4CompassChordAb = $.geometry.segment("curve4CompassChordAb", {
    start: point3CompassTipA.point,
    end: point4CompassTipB.point,
    branchDirection: [-0.5, 0.8660254037844386],
    label: "Compass chord AB",
    role: "profile",
  });
  const constraint1CompassPivotFixed = $.constraint.fixedPoint("constraint1CompassPivotFixed", {
    point: point1CompassPivotO.point,
    target: [0, 0],
    label: "Compass pivot fixed",
  });
  const constraint2CompassBisectorFixed = $.constraint.fixedPoint("constraint2CompassBisectorFixed", {
    point: point2CompassFixedBisectorK.point,
    target: [4.330127018922193, 2.5],
    label: "Compass bisector fixed",
  });
  const constraint3CompassSymmetricTips = $.constraint.symmetricAboutLine("constraint3CompassSymmetricTips", {
    first: point3CompassTipA.point,
    second: point4CompassTipB.point,
    axis: curve1CompassSymmetryAxis.span,
    label: "Compass symmetric tips",
  });
  const constraint4CompassEqualArms = $.constraint.equalLength("constraint4CompassEqualArms", {
    first: curve2CompassArmOa.span,
    second: curve3CompassArmOb.span,
    label: "Compass equal arms",
  });
  const dimension1CompassArmLength4 = $.dimension.curveLength("dimension1CompassArmLength4", {
    curve: curve2CompassArmOa.span,
    value: mm(4),
    label: "Compass arm length 4",
    mode: "driving",
  });
  const dimension2CompassSecondArmReference = $.dimension.curveLength("dimension2CompassSecondArmReference", {
    curve: curve3CompassArmOb.span,
    value: mm(4),
    label: "Compass second arm reference",
    mode: "reference",
  });
  const dimension3CompassChordReference = $.dimension.curveLength("dimension3CompassChordReference", {
    curve: curve4CompassChordAb.span,
    value: mm(4),
    label: "Compass chord reference",
    mode: "reference",
  });
  const dimension4CompassOpeningAngle60Deg = $.dimension.orientedAngle("dimension4CompassOpeningAngle60Deg", {
    first: curve2CompassArmOa.span,
    second: curve3CompassArmOb.span,
    value: rad(1.0471975511965976),
    orientation: "counterClockwise",
    label: "Compass opening angle 60 deg",
    mode: "reference",
  });
  $.group("Points", [point1CompassPivotO, point2CompassFixedBisectorK, point3CompassTipA, point4CompassTipB]);
  $.group("Geometry", [curve1CompassSymmetryAxis, curve2CompassArmOa, curve3CompassArmOb, curve4CompassChordAb]);
  $.group("Constraints", [constraint1CompassPivotFixed, constraint2CompassBisectorFixed, constraint3CompassSymmetricTips, constraint4CompassEqualArms]);
  $.group("Dimensions", [dimension1CompassArmLength4, dimension2CompassSecondArmReference, dimension3CompassChordReference, dimension4CompassOpeningAngle60Deg]);
  return {};
});
