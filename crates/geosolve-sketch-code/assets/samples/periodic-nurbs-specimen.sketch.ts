"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1PeriodicNurbsControl1 = $.geometry.sketchPoint("point1PeriodicNurbsControl1", {
    point: [0, 0],
    label: "Periodic NURBS control 1",
  });
  const point2PeriodicNurbsControl2 = $.geometry.sketchPoint("point2PeriodicNurbsControl2", {
    point: [1.5, -0.2],
    label: "Periodic NURBS control 2",
  });
  const point3PeriodicNurbsControl3 = $.geometry.sketchPoint("point3PeriodicNurbsControl3", {
    point: [2, 1.4],
    label: "Periodic NURBS control 3",
  });
  const point4PeriodicNurbsControl4 = $.geometry.sketchPoint("point4PeriodicNurbsControl4", {
    point: [0.5, 2.2],
    label: "Periodic NURBS control 4",
  });
  const point5PeriodicNurbsControl5 = $.geometry.sketchPoint("point5PeriodicNurbsControl5", {
    point: [-0.8, 1],
    label: "Periodic NURBS control 5",
  });
  const point6PeriodicNurbsSeamWitness = $.geometry.sketchPoint("point6PeriodicNurbsSeamWitness", {
    point: [0.8571428571428568, -0.1142857142857141],
    label: "Periodic NURBS seam witness",
  });
  const curve1PeriodicNurbsSpanAndWindingLab = $.geometry.periodicControlNurbs("curve1PeriodicNurbsSpanAndWindingLab", {
    controls: [{
      key: "control1",
      position: point1PeriodicNurbsControl1.point,
      weight: 0.75,
    }, {
      key: "control2",
      position: point2PeriodicNurbsControl2.point,
      weight: 1,
    }, {
      key: "control3",
      position: point3PeriodicNurbsControl3.point,
      weight: 1.4,
    }, {
      key: "control4",
      position: point4PeriodicNurbsControl4.point,
      weight: 0.9,
    }, {
      key: "control5",
      position: point5PeriodicNurbsControl5.point,
      weight: 1.2,
    }],
    degree: 2,
    gauge: "control2",
    label: "Periodic NURBS span and winding lab",
    role: "profile",
  });
  const constraint1PeriodicNurbsAnchorFixed = $.constraint.fixedPoint("constraint1PeriodicNurbsAnchorFixed", {
    point: point1PeriodicNurbsControl1.point,
    target: [0, 0],
    label: "Periodic NURBS anchor fixed",
  });
  const constraint2PeriodicNurbsWitnessOnExplicitSpan = $.constraint.pointOnCurve("constraint2PeriodicNurbsWitnessOnExplicitSpan", {
    point: point6PeriodicNurbsSeamWitness.point,
    curve: curve1PeriodicNurbsSpanAndWindingLab.spans.byKey["control5"],
    contact: {
      parameter: 1,
      winding: 2,
      neighborhood: {
        kind: "end",
      },
      orientation: "none",
    },
    label: "Periodic NURBS witness on explicit span",
  });
  $.group("Points", [point1PeriodicNurbsControl1, point2PeriodicNurbsControl2, point3PeriodicNurbsControl3, point4PeriodicNurbsControl4, point5PeriodicNurbsControl5, point6PeriodicNurbsSeamWitness]);
  $.group("Geometry", [curve1PeriodicNurbsSpanAndWindingLab]);
  $.group("Constraints", [constraint1PeriodicNurbsAnchorFixed, constraint2PeriodicNurbsWitnessOnExplicitSpan]);
  return {};
});
