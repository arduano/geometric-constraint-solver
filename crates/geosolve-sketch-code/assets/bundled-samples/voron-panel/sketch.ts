"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // One continuous panel boundary follows the pinned DXF topology.
  // The T passage opens at the top edge and both side notches open at the sides.
  // 122 x 37 overall bounds, 10 mm neck, 30 x 10 crossbar extent, and
  // 3.5 mm side-notch mouths use source dimensions. Filleted transitions and
  // semicircular lobes are squared off deliberately; this is not a cut-ready DXF.
  const panelProfile = $.geometry.polyline("panelProfile", {
    vertices: [{
      key: "p0",
      position: [-5, 18.5],
    }, {
      key: "p1",
      position: [-61, 18.5],
    }, {
      key: "p2",
      position: [-61, 3.75],
    }, {
      key: "p3",
      position: [-56.25, 3.75],
    }, {
      key: "p4",
      position: [-56.25, 0.25],
    }, {
      key: "p5",
      position: [-61, 0.25],
    }, {
      key: "p6",
      position: [-61, -18.5],
    }, {
      key: "p7",
      position: [61, -18.5],
    }, {
      key: "p8",
      position: [61, 0.25],
    }, {
      key: "p9",
      position: [56.25, 0.25],
    }, {
      key: "p10",
      position: [56.25, 3.75],
    }, {
      key: "p11",
      position: [61, 3.75],
    }, {
      key: "p12",
      position: [61, 18.5],
    }, {
      key: "p13",
      position: [5, 18.5],
    }, {
      key: "p14",
      position: [5, 5],
    }, {
      key: "p15",
      position: [15, 5],
    }, {
      key: "p16",
      position: [15, -5],
    }, {
      key: "p17",
      position: [-15, -5],
    }, {
      key: "p18",
      position: [-15, 5],
    }, {
      key: "p19",
      position: [-5, 5],
    }],
    closed: true,
    label: "Continuous panel with open T-passage and edge notches",
    role: "profile",
  });
  const edge0 = $.constraint.horizontalPoints("edge0", {
    first: panelProfile.vertices.byKey.p0,
    second: panelProfile.vertices.byKey.p1,
    label: "Panel orthogonal boundary",
  });
  const edge1 = $.constraint.verticalPoints("edge1", {
    first: panelProfile.vertices.byKey.p1,
    second: panelProfile.vertices.byKey.p2,
    label: "Panel orthogonal boundary",
  });
  const edge2 = $.constraint.horizontalPoints("edge2", {
    first: panelProfile.vertices.byKey.p2,
    second: panelProfile.vertices.byKey.p3,
    label: "Panel orthogonal boundary",
  });
  const edge3 = $.constraint.verticalPoints("edge3", {
    first: panelProfile.vertices.byKey.p3,
    second: panelProfile.vertices.byKey.p4,
    label: "Panel orthogonal boundary",
  });
  const edge4 = $.constraint.horizontalPoints("edge4", {
    first: panelProfile.vertices.byKey.p4,
    second: panelProfile.vertices.byKey.p5,
    label: "Panel orthogonal boundary",
  });
  const edge5 = $.constraint.verticalPoints("edge5", {
    first: panelProfile.vertices.byKey.p5,
    second: panelProfile.vertices.byKey.p6,
    label: "Panel orthogonal boundary",
  });
  const edge6 = $.constraint.horizontalPoints("edge6", {
    first: panelProfile.vertices.byKey.p6,
    second: panelProfile.vertices.byKey.p7,
    label: "Panel orthogonal boundary",
  });
  const edge7 = $.constraint.verticalPoints("edge7", {
    first: panelProfile.vertices.byKey.p7,
    second: panelProfile.vertices.byKey.p8,
    label: "Panel orthogonal boundary",
  });
  const edge8 = $.constraint.horizontalPoints("edge8", {
    first: panelProfile.vertices.byKey.p8,
    second: panelProfile.vertices.byKey.p9,
    label: "Panel orthogonal boundary",
  });
  const edge9 = $.constraint.verticalPoints("edge9", {
    first: panelProfile.vertices.byKey.p9,
    second: panelProfile.vertices.byKey.p10,
    label: "Panel orthogonal boundary",
  });
  const edge10 = $.constraint.horizontalPoints("edge10", {
    first: panelProfile.vertices.byKey.p10,
    second: panelProfile.vertices.byKey.p11,
    label: "Panel orthogonal boundary",
  });
  const edge11 = $.constraint.verticalPoints("edge11", {
    first: panelProfile.vertices.byKey.p11,
    second: panelProfile.vertices.byKey.p12,
    label: "Panel orthogonal boundary",
  });
  const edge12 = $.constraint.horizontalPoints("edge12", {
    first: panelProfile.vertices.byKey.p12,
    second: panelProfile.vertices.byKey.p13,
    label: "Panel orthogonal boundary",
  });
  const edge13 = $.constraint.verticalPoints("edge13", {
    first: panelProfile.vertices.byKey.p13,
    second: panelProfile.vertices.byKey.p14,
    label: "Panel orthogonal boundary",
  });
  const edge14 = $.constraint.horizontalPoints("edge14", {
    first: panelProfile.vertices.byKey.p14,
    second: panelProfile.vertices.byKey.p15,
    label: "Panel orthogonal boundary",
  });
  const edge15 = $.constraint.verticalPoints("edge15", {
    first: panelProfile.vertices.byKey.p15,
    second: panelProfile.vertices.byKey.p16,
    label: "Panel orthogonal boundary",
  });
  const edge16 = $.constraint.horizontalPoints("edge16", {
    first: panelProfile.vertices.byKey.p16,
    second: panelProfile.vertices.byKey.p17,
    label: "Panel orthogonal boundary",
  });
  const edge17 = $.constraint.verticalPoints("edge17", {
    first: panelProfile.vertices.byKey.p17,
    second: panelProfile.vertices.byKey.p18,
    label: "Panel orthogonal boundary",
  });
  const edge18 = $.constraint.horizontalPoints("edge18", {
    first: panelProfile.vertices.byKey.p18,
    second: panelProfile.vertices.byKey.p19,
    label: "Panel orthogonal boundary",
  });
  const edge19 = $.constraint.verticalPoints("edge19", {
    first: panelProfile.vertices.byKey.p19,
    second: panelProfile.vertices.byKey.p0,
    label: "Panel orthogonal boundary",
  });
  const symmetry0 = $.constraint.symmetricAboutDatumAxis("symmetry0", {
    first: panelProfile.vertices.byKey.p0,
    second: panelProfile.vertices.byKey.p13,
    axis: "y",
    label: "Panel bilateral symmetry",
  });
  const symmetry1 = $.constraint.symmetricAboutDatumAxis("symmetry1", {
    first: panelProfile.vertices.byKey.p1,
    second: panelProfile.vertices.byKey.p12,
    axis: "y",
    label: "Panel bilateral symmetry",
  });
  const symmetry2 = $.constraint.symmetricAboutDatumAxis("symmetry2", {
    first: panelProfile.vertices.byKey.p2,
    second: panelProfile.vertices.byKey.p11,
    axis: "y",
    label: "Panel bilateral symmetry",
  });
  const symmetry3 = $.constraint.symmetricAboutDatumAxis("symmetry3", {
    first: panelProfile.vertices.byKey.p3,
    second: panelProfile.vertices.byKey.p10,
    axis: "y",
    label: "Panel bilateral symmetry",
  });
  const symmetry4 = $.constraint.symmetricAboutDatumAxis("symmetry4", {
    first: panelProfile.vertices.byKey.p4,
    second: panelProfile.vertices.byKey.p9,
    axis: "y",
    label: "Panel bilateral symmetry",
  });
  const symmetry5 = $.constraint.symmetricAboutDatumAxis("symmetry5", {
    first: panelProfile.vertices.byKey.p5,
    second: panelProfile.vertices.byKey.p8,
    axis: "y",
    label: "Panel bilateral symmetry",
  });
  const symmetry6 = $.constraint.symmetricAboutDatumAxis("symmetry6", {
    first: panelProfile.vertices.byKey.p6,
    second: panelProfile.vertices.byKey.p7,
    axis: "y",
    label: "Panel bilateral symmetry",
  });
  const symmetry17 = $.constraint.symmetricAboutDatumAxis("symmetry17", {
    first: panelProfile.vertices.byKey.p17,
    second: panelProfile.vertices.byKey.p16,
    axis: "y",
    label: "Panel bilateral symmetry",
  });
  const symmetry18 = $.constraint.symmetricAboutDatumAxis("symmetry18", {
    first: panelProfile.vertices.byKey.p18,
    second: panelProfile.vertices.byKey.p15,
    axis: "y",
    label: "Panel bilateral symmetry",
  });
  const symmetry19 = $.constraint.symmetricAboutDatumAxis("symmetry19", {
    first: panelProfile.vertices.byKey.p19,
    second: panelProfile.vertices.byKey.p14,
    axis: "y",
    label: "Panel bilateral symmetry",
  });
  const panelHalfWidth = $.constraint.fixedCoordinate("panelHalfWidth", {
    point: panelProfile.vertices.byKey.p12,
    axis: "x",
    target: mm(61),
    label: "panelHalfWidth design datum",
  });
  const panelTop = $.constraint.fixedCoordinate("panelTop", {
    point: panelProfile.vertices.byKey.p12,
    axis: "y",
    target: mm(18.5),
    label: "panelTop design datum",
  });
  const panelBottom = $.constraint.fixedCoordinate("panelBottom", {
    point: panelProfile.vertices.byKey.p7,
    axis: "y",
    target: mm(-18.5),
    label: "panelBottom design datum",
  });
  const neckHalfWidth = $.constraint.fixedCoordinate("neckHalfWidth", {
    point: panelProfile.vertices.byKey.p13,
    axis: "x",
    target: mm(5),
    label: "neckHalfWidth design datum",
  });
  const passageHalfWidth = $.constraint.fixedCoordinate("passageHalfWidth", {
    point: panelProfile.vertices.byKey.p15,
    axis: "x",
    target: mm(15),
    label: "passageHalfWidth design datum",
  });
  const passageTop = $.constraint.fixedCoordinate("passageTop", {
    point: panelProfile.vertices.byKey.p15,
    axis: "y",
    target: mm(5),
    label: "passageTop design datum",
  });
  const passageBottom = $.constraint.fixedCoordinate("passageBottom", {
    point: panelProfile.vertices.byKey.p16,
    axis: "y",
    target: mm(-5),
    label: "passageBottom design datum",
  });
  const notchInner = $.constraint.fixedCoordinate("notchInner", {
    point: panelProfile.vertices.byKey.p9,
    axis: "x",
    target: mm(56.25),
    label: "notchInner design datum",
  });
  const notchTop = $.constraint.fixedCoordinate("notchTop", {
    point: panelProfile.vertices.byKey.p10,
    axis: "y",
    target: mm(3.75),
    label: "notchTop design datum",
  });
  const notchBottom = $.constraint.fixedCoordinate("notchBottom", {
    point: panelProfile.vertices.byKey.p9,
    axis: "y",
    target: mm(0.25),
    label: "notchBottom design datum",
  });
  const alignedOuterSides = $.constraint.verticalPoints("alignedOuterSides", {
    first: panelProfile.vertices.byKey.p1,
    second: panelProfile.vertices.byKey.p5,
    label: "Upper and lower outside walls remain aligned across the notches",
  });
  const neckWidthDatum = $.geometry.segment("neckWidthDatum", {
    start: panelProfile.vertices.byKey.p0,
    end: panelProfile.vertices.byKey.p13,
    branchDirection: [1, 0],
    role: "construction",
    label: "Passage mouth width datum",
  });
  const leftNotchDatum = $.geometry.polyline("leftNotchDatum", {
    vertices: [{
      key: "mouthTop",
      position: panelProfile.vertices.byKey.p2,
    }, {
      key: "innerTop",
      position: panelProfile.vertices.byKey.p3,
    }, {
      key: "innerBottom",
      position: panelProfile.vertices.byKey.p4,
    }, {
      key: "mouthBottom",
      position: panelProfile.vertices.byKey.p5,
    }],
    role: "construction",
    label: "Left edge-notch reference",
  });
  const rightNotchDatum = $.geometry.polyline("rightNotchDatum", {
    vertices: [{
      key: "mouthBottom",
      position: panelProfile.vertices.byKey.p8,
    }, {
      key: "innerBottom",
      position: panelProfile.vertices.byKey.p9,
    }, {
      key: "innerTop",
      position: panelProfile.vertices.byKey.p10,
    }, {
      key: "mouthTop",
      position: panelProfile.vertices.byKey.p11,
    }],
    role: "construction",
    label: "Right edge-notch reference",
  });
  const overallWidth = $.dimension.pointDistance("overallWidth", {
    first: panelProfile.vertices.byKey.p6,
    second: panelProfile.vertices.byKey.p7,
    value: mm(122),
    mode: "reference",
    label: "Measured panel width",
  });
  const overallHeight = $.dimension.pointDistance("overallHeight", {
    first: panelProfile.vertices.byKey.p1,
    second: panelProfile.vertices.byKey.p6,
    value: mm(37),
    mode: "reference",
    label: "Measured panel height",
  });
  const neckWidth = $.dimension.curveLength("neckWidth", {
    curve: neckWidthDatum.span,
    value: mm(10),
    mode: "reference",
    label: "Measured passage mouth",
  });
  $.group("Panel envelope", [panelProfile, edge0, edge1, edge2, edge3, edge4, edge5, edge6, edge7, edge8, edge9, edge10, edge11, edge12, edge13, edge14, edge15, edge16, edge17, edge18, edge19, symmetry0, symmetry1, symmetry2, symmetry3, symmetry4, symmetry5, symmetry6, symmetry17, symmetry18, symmetry19, panelHalfWidth, panelTop, panelBottom, alignedOuterSides, overallWidth, overallHeight]);
  $.group("Central motor passage", [neckHalfWidth, passageHalfWidth, passageTop, passageBottom, neckWidthDatum, neckWidth]);
  $.group("Side-notch references", [notchInner, notchTop, notchBottom, leftNotchDatum, rightNotchDatum]);
  return {};
});
