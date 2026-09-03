"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1OrbitFixedCenter = $.geometry.sketchPoint("point1OrbitFixedCenter", {
    point: [0, 0],
    label: "Orbit fixed center",
  });
  const point2OrbitSatelliteCenter = $.geometry.sketchPoint("point2OrbitSatelliteCenter", {
    point: [4, 0],
    label: "Orbit satellite center",
  });
  const curve1OrbitFixedCircle = $.geometry.centerRadiusCircle("curve1OrbitFixedCircle", {
    center: point1OrbitFixedCenter.point,
    radius: mm(3),
    label: "Orbit fixed circle",
    role: "profile",
  });
  const curve2OrbitTangentSatellite = $.geometry.centerRadiusCircle("curve2OrbitTangentSatellite", {
    center: point2OrbitSatelliteCenter.point,
    radius: mm(1),
    label: "Orbit tangent satellite",
    role: "profile",
  });
  const constraint1OrbitCenterFixed = $.constraint.fixedPoint("constraint1OrbitCenterFixed", {
    point: point1OrbitFixedCenter.point,
    target: [0, 0],
    label: "Orbit center fixed",
  });
  const dimension1OrbitFixedRadius3 = $.dimension.radius("dimension1OrbitFixedRadius3", {
    curve: curve1OrbitFixedCircle.curve,
    value: mm(3),
    label: "Orbit fixed radius 3",
    mode: "driving",
  });
  const dimension2OrbitSatelliteRadius1 = $.dimension.radius("dimension2OrbitSatelliteRadius1", {
    curve: curve2OrbitTangentSatellite.curve,
    value: mm(1),
    label: "Orbit satellite radius 1",
    mode: "driving",
  });
  const constraint2OrbitExternalTangency = $.constraint.curveCurveTangency("constraint2OrbitExternalTangency", {
    first: curve1OrbitFixedCircle.span,
    second: curve2OrbitTangentSatellite.span,
    contacts: {
      first: {
        parameter: 0,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "opposed",
      },
      second: {
        parameter: 3.141592653589793,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "opposed",
      },
    },
    label: "Orbit external tangency",
  });
  const dimension3OrbitCenterDistanceReference = $.dimension.pointDistance("dimension3OrbitCenterDistanceReference", {
    first: point1OrbitFixedCenter.point,
    second: point2OrbitSatelliteCenter.point,
    value: mm(4),
    label: "Orbit center distance reference",
    mode: "reference",
  });
  $.group("Points", [point1OrbitFixedCenter, point2OrbitSatelliteCenter]);
  $.group("Geometry", [curve1OrbitFixedCircle, curve2OrbitTangentSatellite]);
  $.group("Constraints", [constraint1OrbitCenterFixed, constraint2OrbitExternalTangency]);
  $.group("Dimensions", [dimension1OrbitFixedRadius3, dimension2OrbitSatelliteRadius1, dimension3OrbitCenterDistanceReference]);
  return {};
});
