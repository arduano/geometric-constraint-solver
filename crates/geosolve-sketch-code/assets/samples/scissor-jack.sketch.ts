"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1ScissorFixedAnchorA = $.geometry.sketchPoint("point1ScissorFixedAnchorA", {
    point: [-4, 0],
    label: "Scissor fixed anchor A",
  });
  const point2ScissorBaseSliderB = $.geometry.sketchPoint("point2ScissorBaseSliderB", {
    point: [4, 0],
    label: "Scissor base slider B",
  });
  const point3ScissorUpperJointU = $.geometry.sketchPoint("point3ScissorUpperJointU", {
    point: [0, 3],
    label: "Scissor upper joint U",
  });
  const point4ScissorLowerJointL = $.geometry.sketchPoint("point4ScissorLowerJointL", {
    point: [0, -3],
    label: "Scissor lower joint L",
  });
  const curve1ScissorSlidingBaseAb = $.geometry.segment("curve1ScissorSlidingBaseAb", {
    start: point1ScissorFixedAnchorA.point,
    end: point2ScissorBaseSliderB.point,
    branchDirection: [1, 0],
    label: "Scissor sliding base AB",
    role: "profile",
  });
  const curve2ScissorUpperLeftArmAu = $.geometry.segment("curve2ScissorUpperLeftArmAu", {
    start: point1ScissorFixedAnchorA.point,
    end: point3ScissorUpperJointU.point,
    branchDirection: [0.8, 0.6],
    label: "Scissor upper-left arm AU",
    role: "profile",
  });
  const curve3ScissorUpperRightArmUb = $.geometry.segment("curve3ScissorUpperRightArmUb", {
    start: point3ScissorUpperJointU.point,
    end: point2ScissorBaseSliderB.point,
    branchDirection: [0.8, -0.6],
    label: "Scissor upper-right arm UB",
    role: "profile",
  });
  const curve4ScissorLowerLeftArmAl = $.geometry.segment("curve4ScissorLowerLeftArmAl", {
    start: point1ScissorFixedAnchorA.point,
    end: point4ScissorLowerJointL.point,
    branchDirection: [0.8, -0.6],
    label: "Scissor lower-left arm AL",
    role: "profile",
  });
  const curve5ScissorLowerRightArmLb = $.geometry.segment("curve5ScissorLowerRightArmLb", {
    start: point4ScissorLowerJointL.point,
    end: point2ScissorBaseSliderB.point,
    branchDirection: [0.8, 0.6],
    label: "Scissor lower-right arm LB",
    role: "profile",
  });
  const constraint1ScissorAnchorFixed = $.constraint.fixedPoint("constraint1ScissorAnchorFixed", {
    point: point1ScissorFixedAnchorA.point,
    target: [-4, 0],
    label: "Scissor anchor fixed",
  });
  const constraint2ScissorBSlidesHorizontally = $.constraint.fixedCoordinate("constraint2ScissorBSlidesHorizontally", {
    point: point2ScissorBaseSliderB.point,
    axis: "y",
    target: mm(0),
    label: "Scissor B slides horizontally",
  });
  const dimension1ScissorArmLength5 = $.dimension.curveLength("dimension1ScissorArmLength5", {
    curve: curve2ScissorUpperLeftArmAu.span,
    value: mm(5),
    label: "Scissor arm length 5",
    mode: "driving",
  });
  const constraint3ScissorUpperArmsEqual = $.constraint.equalLength("constraint3ScissorUpperArmsEqual", {
    first: curve2ScissorUpperLeftArmAu.span,
    second: curve3ScissorUpperRightArmUb.span,
    label: "Scissor upper arms equal",
  });
  const constraint4ScissorJointsMirrorAcrossBase = $.constraint.symmetricAboutLine("constraint4ScissorJointsMirrorAcrossBase", {
    first: point3ScissorUpperJointU.point,
    second: point4ScissorLowerJointL.point,
    axis: curve1ScissorSlidingBaseAb.span,
    label: "Scissor joints mirror across base",
  });
  $.group("Points", [point1ScissorFixedAnchorA, point2ScissorBaseSliderB, point3ScissorUpperJointU, point4ScissorLowerJointL]);
  $.group("Geometry", [curve1ScissorSlidingBaseAb, curve2ScissorUpperLeftArmAu, curve3ScissorUpperRightArmUb, curve4ScissorLowerLeftArmAl, curve5ScissorLowerRightArmLb]);
  $.group("Constraints", [constraint1ScissorAnchorFixed, constraint2ScissorBSlidesHorizontally, constraint3ScissorUpperArmsEqual, constraint4ScissorJointsMirrorAcrossBase]);
  $.group("Dimensions", [dimension1ScissorArmLength5]);
  return {};
});
