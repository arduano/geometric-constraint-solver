"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch({
  title: "Theo Jansen-style walking leg · 1 DOF",
  description: "The articulated Jansen eight-bar leg turns a complete crank revolution into a broad, nearly level stance and lifted return.",
}, ($) => {
  // Jansen eight-bar topology: rigid A-C-E and D-F-G triangles are coupled
  // by E-F and driven separately by B-C and B-D. G is a free output.
  // Published dimensionless Jansen ratios are interpreted as millimetres.
  // The initial assembly has C above A-B, D below A-B, E left of A-C,
  // F below E-D and G below D-F; the retained motion regression checks all five.
  const groundPivot = $.geometry.sketchPoint("groundPivot", {
    point: [0, 0],
    label: "Crank ground O",
  });
  const framePivot = $.geometry.sketchPoint("framePivot", {
    point: [-38, -7.8],
    label: "Frame pivot A",
  });
  const crankPin = $.geometry.sketchPoint("crankPin", {
    point: [15, 0],
    label: "Crank pin B",
  });
  const upperKnee = $.geometry.sketchPoint("upperKnee", {
    point: [-24.013535097127793, 31.27209745484268],
    label: "Upper knee C",
  });
  const lowerKnee = $.geometry.sketchPoint("lowerKnee", {
    point: [-26.952107031572957, -45.51517017008116],
    label: "Lower knee D",
  });
  const upperAnkle = $.geometry.sketchPoint("upperAnkle", {
    point: [-74.79436538093606, 8.143170205896142],
    label: "Upper ankle E",
  });
  const lowerAnkle = $.geometry.sketchPoint("lowerAnkle", {
    point: [-59.231514961414995, -28.052930230747755],
    label: "Lower ankle F",
  });
  const foot = $.geometry.sketchPoint("foot", {
    point: [-43.160110524104724, -91.75693292612323],
    label: "Foot output G",
  });
  const frame = $.geometry.segment("frame", {
    start: groundPivot.point,
    end: framePivot.point,
    branchDirection: [-0.9795766701349669, -0.20107100071191422],
    label: "frame",
    role: "construction",
  });
  const crank = $.geometry.segment("crank", {
    start: groundPivot.point,
    end: crankPin.point,
    branchDirection: [1, 0],
    label: "crank",
    role: "profile",
  });
  const crankLength = $.dimension.curveLength("crankLength", {
    isKeyConstraint: true,
    curve: crank.span,
    value: mm(15),
    label: "crank length",
    mode: "driving",
  });
  const upperRocker = $.geometry.segment("upperRocker", {
    start: framePivot.point,
    end: upperKnee.point,
    branchDirection: [0.3370232506716195, 0.9414963242130767],
    label: "upperRocker",
    role: "profile",
  });
  const upperRockerLength = $.dimension.curveLength("upperRockerLength", {
    isKeyConstraint: true,
    curve: upperRocker.span,
    value: mm(41.5),
    label: "upperRocker length",
    mode: "driving",
  });
  const crankCoupler = $.geometry.segment("crankCoupler", {
    start: crankPin.point,
    end: upperKnee.point,
    branchDirection: [-0.7802707019425559, 0.6254419490968536],
    label: "crankCoupler",
    role: "profile",
  });
  const crankCouplerLength = $.dimension.curveLength("crankCouplerLength", {
    curve: crankCoupler.span,
    value: mm(50),
    label: "crankCoupler length",
    mode: "driving",
  });
  const lowerRocker = $.geometry.segment("lowerRocker", {
    start: framePivot.point,
    end: lowerKnee.point,
    branchDirection: [0.2811168694256245, -0.9596735412234394],
    label: "lowerRocker",
    role: "profile",
  });
  const lowerRockerLength = $.dimension.curveLength("lowerRockerLength", {
    isKeyConstraint: true,
    curve: lowerRocker.span,
    value: mm(39.3),
    label: "lowerRocker length",
    mode: "driving",
  });
  const lowerCrankCoupler = $.geometry.segment("lowerCrankCoupler", {
    start: crankPin.point,
    end: lowerKnee.point,
    branchDirection: [-0.6777400166651529, -0.7353016182565617],
    label: "lowerCrankCoupler",
    role: "profile",
  });
  const lowerCrankCouplerLength = $.dimension.curveLength("lowerCrankCouplerLength", {
    isKeyConstraint: true,
    curve: lowerCrankCoupler.span,
    value: mm(61.9),
    label: "lowerCrankCoupler length",
    mode: "driving",
  });
  const upperTriangleBase = $.geometry.segment("upperTriangleBase", {
    start: framePivot.point,
    end: upperAnkle.point,
    branchDirection: [-0.9175652214697271, 0.3975852919176095],
    label: "upperTriangleBase",
    role: "profile",
  });
  const upperTriangleBaseLength = $.dimension.curveLength("upperTriangleBaseLength", {
    curve: upperTriangleBase.span,
    value: mm(40.1),
    label: "upperTriangleBase length",
    mode: "driving",
  });
  const upperTriangleBrace = $.geometry.segment("upperTriangleBrace", {
    start: upperKnee.point,
    end: upperAnkle.point,
    branchDirection: [-0.9100507219320477, -0.41449690410298456],
    label: "upperTriangleBrace",
    role: "profile",
  });
  const upperTriangleBraceLength = $.dimension.curveLength("upperTriangleBraceLength", {
    curve: upperTriangleBrace.span,
    value: mm(55.8),
    label: "upperTriangleBrace length",
    mode: "driving",
  });
  const ankleLink = $.geometry.segment("ankleLink", {
    start: upperAnkle.point,
    end: lowerAnkle.point,
    branchDirection: [0.3949962035411438, -0.9186827521990838],
    label: "ankleLink",
    role: "profile",
  });
  const ankleLinkLength = $.dimension.curveLength("ankleLinkLength", {
    curve: ankleLink.span,
    value: mm(39.4),
    label: "ankleLink length",
    mode: "driving",
  });
  const lowerTriangleBase = $.geometry.segment("lowerTriangleBase", {
    start: lowerKnee.point,
    end: lowerAnkle.point,
    branchDirection: [-0.8795478999956959, 0.4758103525703926],
    label: "lowerTriangleBase",
    role: "profile",
  });
  const lowerTriangleBaseLength = $.dimension.curveLength("lowerTriangleBaseLength", {
    curve: lowerTriangleBase.span,
    value: mm(36.7),
    label: "lowerTriangleBase length",
    mode: "driving",
  });
  const lowerFootLink = $.geometry.segment("lowerFootLink", {
    start: lowerKnee.point,
    end: foot.point,
    branchDirection: [-0.33077558148024006, -0.9437094440008583],
    label: "lowerFootLink",
    role: "profile",
  });
  const lowerFootLinkLength = $.dimension.curveLength("lowerFootLinkLength", {
    curve: lowerFootLink.span,
    value: mm(49),
    label: "lowerFootLink length",
    mode: "driving",
  });
  const upperFootLink = $.geometry.segment("upperFootLink", {
    start: lowerAnkle.point,
    end: foot.point,
    branchDirection: [0.24461802796514864, -0.9696195235216964],
    label: "upperFootLink",
    role: "profile",
  });
  const upperFootLinkLength = $.dimension.curveLength("upperFootLinkLength", {
    isKeyConstraint: true,
    curve: upperFootLink.span,
    value: mm(65.7),
    label: "upperFootLink length",
    mode: "driving",
  });
  const groundAnchor = $.constraint.fixedPoint("groundAnchor", {
    point: groundPivot.point,
    target: [0, 0],
    label: "Crank ground O datum",
  });
  const frameAnchor = $.constraint.fixedPoint("frameAnchor", {
    point: framePivot.point,
    target: [-38, -7.8],
    label: "Frame pivot A datum",
  });
  $.group("Ground frame", [groundPivot, framePivot, frame, groundAnchor, frameAnchor]);
  $.group("Crank input", [crankPin, crank, crankLength]);
  $.group("Coupled leg", [upperKnee, lowerKnee, upperAnkle, lowerAnkle, foot, upperRocker, upperRockerLength, crankCoupler, crankCouplerLength, lowerRocker, lowerRockerLength, lowerCrankCoupler, lowerCrankCouplerLength, upperTriangleBase, upperTriangleBaseLength, upperTriangleBrace, upperTriangleBraceLength, ankleLink, ankleLinkLength, lowerTriangleBase, lowerTriangleBaseLength, lowerFootLink, lowerFootLinkLength, upperFootLink, upperFootLinkLength]);
  return {};
});
