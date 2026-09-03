"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1CamQ0 = $.geometry.sketchPoint("point1CamQ0", {
    point: [-4, 0],
    label: "Cam Q0",
  });
  const point2CamQ1 = $.geometry.sketchPoint("point2CamQ1", {
    point: [0, 4],
    label: "Cam Q1",
  });
  const point3CamQ2 = $.geometry.sketchPoint("point3CamQ2", {
    point: [4, 0],
    label: "Cam Q2",
  });
  const point4LeftRollerCenter = $.geometry.sketchPoint("point4LeftRollerCenter", {
    point: [-2.447213595499958, 2.3944271909999157],
    label: "Left roller center",
  });
  const point5RightRollerCenter = $.geometry.sketchPoint("point5RightRollerCenter", {
    point: [2.447213595499958, 2.3944271909999157],
    label: "Right roller center",
  });
  const curve1QuadraticBezierCam = $.geometry.quadraticBezier("curve1QuadraticBezierCam", {
    start: point1CamQ0.point,
    control: point2CamQ1.point,
    end: point3CamQ2.point,
    label: "Quadratic Bezier cam",
    role: "profile",
  });
  const curve2LeftCamRoller = $.geometry.centerRadiusCircle("curve2LeftCamRoller", {
    center: point4LeftRollerCenter.point,
    radius: mm(1),
    label: "Left cam roller",
    role: "profile",
  });
  const curve3RightCamRoller = $.geometry.centerRadiusCircle("curve3RightCamRoller", {
    center: point5RightRollerCenter.point,
    radius: mm(1),
    label: "Right cam roller",
    role: "profile",
  });
  const constraint1CamQ0Fixed = $.constraint.fixedPoint("constraint1CamQ0Fixed", {
    point: point1CamQ0.point,
    target: [-4, 0],
    label: "Cam Q0 fixed",
  });
  const constraint2CamQ1Fixed = $.constraint.fixedPoint("constraint2CamQ1Fixed", {
    point: point2CamQ1.point,
    target: [0, 4],
    label: "Cam Q1 fixed",
  });
  const constraint3CamQ2Fixed = $.constraint.fixedPoint("constraint3CamQ2Fixed", {
    point: point3CamQ2.point,
    target: [4, 0],
    label: "Cam Q2 fixed",
  });
  const dimension1CamRollerRadius1 = $.dimension.radius("dimension1CamRollerRadius1", {
    curve: curve2LeftCamRoller.curve,
    value: mm(1),
    label: "Cam roller radius 1",
    mode: "driving",
  });
  const constraint4CamRollersEqualRadius = $.constraint.equalRadius("constraint4CamRollersEqualRadius", {
    first: curve2LeftCamRoller.curve,
    second: curve3RightCamRoller.curve,
    label: "Cam rollers equal radius",
  });
  const constraint5LeftRollerTangentToCam = $.constraint.curveCurveTangency("constraint5LeftRollerTangentToCam", {
    first: curve1QuadraticBezierCam.span,
    second: curve2LeftCamRoller.span,
    contacts: {
      first: {
        parameter: 0.25,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "aligned",
      },
      second: {
        parameter: 5.176036589385496,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "aligned",
      },
    },
    label: "Left roller tangent to cam",
  });
  const constraint6RightRollerTangentToCam = $.constraint.curveCurveTangency("constraint6RightRollerTangentToCam", {
    first: curve1QuadraticBezierCam.span,
    second: curve3RightCamRoller.span,
    contacts: {
      first: {
        parameter: 0.75,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "aligned",
      },
      second: {
        parameter: 4.2487413713838835,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "aligned",
      },
    },
    label: "Right roller tangent to cam",
  });
  const dimension2RightRollerDiameterReference = $.dimension.diameter("dimension2RightRollerDiameterReference", {
    curve: curve3RightCamRoller.curve,
    value: mm(2),
    label: "Right roller diameter reference",
    mode: "reference",
  });
  $.group("Points", [point1CamQ0, point2CamQ1, point3CamQ2, point4LeftRollerCenter, point5RightRollerCenter]);
  $.group("Geometry", [curve1QuadraticBezierCam, curve2LeftCamRoller, curve3RightCamRoller]);
  $.group("Constraints", [constraint1CamQ0Fixed, constraint2CamQ1Fixed, constraint3CamQ2Fixed, constraint4CamRollersEqualRadius, constraint5LeftRollerTangentToCam, constraint6RightRollerTangentToCam]);
  $.group("Dimensions", [dimension1CamRollerRadius1, dimension2RightRollerDiameterReference]);
  return {};
});
