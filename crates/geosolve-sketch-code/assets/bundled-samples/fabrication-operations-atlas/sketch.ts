"use geosolve sketch";
import { deg, sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // Profile generators: these operations deliberately expose their authored
  // width, height, radius, side count and orientation as managed controls.
  const stockBlank = $.operation.rectangle("stockBlank", {
    origin: [-38, 15],
    width: mm(14),
    height: mm(10),
    role: "profile",
    label: "14 × 10 mm stock blank",
  });
  const hexFlange = $.operation.regularPolygon("hexFlange", {
    center: [-13, 20],
    radius: mm(6),
    sides: 6,
    rotation: deg(30),
    role: "profile",
    label: "Hexagonal flange",
  });
  const obroundPort = $.operation.slot("obroundPort", {
    firstCenter: [4, 20],
    secondCenter: [16, 20],
    radius: mm(3),
    role: "profile",
    label: "Machined obround port",
  });
  // Topology operations retain both their native source and their exact
  // operation result so the Explorer can explain what is consumed or kept.
  const splitSource = $.geometry.segment("splitSource", {
    start: [-38, 6],
    end: [-24, 6],
    branchDirection: [1, 0],
    role: "construction",
    label: "Split stock",
  });
  const splitAtDatum = $.operation.split("splitAtDatum", {
    source: splitSource.span,
    parameter: 0.4,
    retained: "before",
    label: "Split at 40%",
  });
  const breakSource = $.geometry.segment("breakSource", {
    start: [-18, 6],
    end: [-4, 6],
    branchDirection: [1, 0],
    role: "construction",
    label: "Break stock",
  });
  const reliefBreak = $.operation.break("reliefBreak", {
    source: breakSource.span,
    start: 0.3,
    end: 0.7,
    retained: "before",
    label: "Central relief break",
  });
  const trimSource = $.geometry.segment("trimSource", {
    start: [2, 6],
    end: [16, 6],
    branchDirection: [1, 0],
    role: "construction",
    label: "Trim stock",
  });
  const finishTrim = $.operation.trim("finishTrim", {
    source: trimSource.span,
    parameter: 0.65,
    retained: "after",
    label: "Retain final 35%",
  });
  const extensionSource = $.geometry.segment("extensionSource", {
    start: [23, 6],
    end: [28, 6],
    branchDirection: [1, 0],
    role: "construction",
    label: "Extension source",
  });
  const extensionLimit = $.geometry.segment("extensionLimit", {
    start: [34, 2],
    end: [34, 10],
    branchDirection: [0, 1],
    role: "construction",
    label: "Extension limit",
  });
  const extendToLimit = $.operation.extend("extendToLimit", {
    source: extensionSource.span,
    target: extensionLimit.span,
    endpoint: "end",
    label: "Extend to vertical limit",
  });
  // Finishing operations demonstrate relational construction: a mirrored
  // curve, two parent-contact operations and an open-chain offset.
  const mirrorAxis = $.geometry.segment("mirrorAxis", {
    start: [-28, -15],
    end: [-28, -2],
    branchDirection: [0, 1],
    role: "construction",
    label: "Mirror centreline",
  });
  const mirrorSeed = $.geometry.quadraticBezier("mirrorSeed", {
    start: [-25, -13],
    control: [-20, -4],
    end: [-15, -12],
    role: "profile",
    label: "Mirror seed profile",
  });
  const reflectedProfile = $.operation.mirror("reflectedProfile", {
    source: mirrorSeed.curve,
    axis: mirrorAxis.span,
    label: "Reflected profile",
  });
  const chamferHorizontal = $.geometry.segment("chamferHorizontal", {
    start: [-8, -13],
    end: [2, -13],
    branchDirection: [1, 0],
    role: "profile",
    label: "Chamfer horizontal parent",
  });
  const chamferVertical = $.geometry.segment("chamferVertical", {
    start: chamferHorizontal.start,
    end: [-8, -3],
    branchDirection: [0, 1],
    role: "profile",
    label: "Chamfer vertical parent",
  });
  const cornerChamfer = $.operation.chamfer("cornerChamfer", {
    first: chamferHorizontal.span,
    second: chamferVertical.span,
    firstDistance: mm(2),
    secondDistance: mm(3),
    label: "2 × 3 mm asymmetric chamfer",
  });
  const filletHorizontal = $.geometry.segment("filletHorizontal", {
    start: [7, -13],
    end: [19, -13],
    branchDirection: [1, 0],
    role: "profile",
    label: "Fillet horizontal parent",
  });
  const filletVertical = $.geometry.segment("filletVertical", {
    start: filletHorizontal.end,
    end: [19, -1],
    branchDirection: [0, 1],
    role: "profile",
    label: "Fillet vertical parent",
  });
  const cornerFillet = $.operation.associativeFillet("cornerFillet", {
    radius: mm(2),
    radiusMode: "driving",
    parents: [{
      span: filletHorizontal.span,
      parameter: 0.75,
      winding: 0,
      neighborhood: {
        kind: "interior",
      },
      normalSide: "left",
      trimEndpoint: "end",
      periodicAnchor: {
        kind: "none",
      },
    }, {
      span: filletVertical.span,
      parameter: 0.25,
      winding: 0,
      neighborhood: {
        kind: "interior",
      },
      normalSide: "left",
      trimEndpoint: "start",
      periodicAnchor: {
        kind: "none",
      },
    }],
    endpointOrder: "firstThenSecond",
    sweep: "counterClockwise",
    label: "Associative 2 mm fillet",
  });
  const offsetSource = $.geometry.segment("offsetSource", {
    start: [25, -13],
    end: [39, -13],
    branchDirection: [1, 0],
    role: "profile",
    label: "Offset source edge",
  });
  const offsetChain = $.aggregate.openChain("offsetChain", {
    spans: [offsetSource.span],
    label: "Open edge chain",
  });
  const machiningAllowance = $.operation.profileOffset("machiningAllowance", {
    sources: [offsetChain.chain],
    distance: mm(2),
    side: "left",
    firstTraversal: "forward",
    label: "2 mm machining allowance",
  });
  // A true generated feature: one authored centre-drill cross becomes
  // twenty-four equally spaced instances. Both sources are native point-
  // defined spans, the exact family admitted by Linear Pattern.
  const pilotHorizontal = $.geometry.segment("pilotHorizontal", {
    start: [-39.4, -24],
    end: [-36.6, -24],
    branchDirection: [1, 0],
    role: "construction",
    label: "Centre-drill horizontal marker",
  });
  const pilotVertical = $.geometry.segment("pilotVertical", {
    start: [-38, -25.4],
    end: [-38, -22.6],
    branchDirection: [0, 1],
    role: "construction",
    label: "Centre-drill vertical marker",
  });
  const holeStrip = $.operation.linearPattern("holeStrip", {
    sources: [pilotHorizontal.curve, pilotVertical.curve],
    instances: 24,
    step: [3.3, 0],
    label: "24-place centre-drill strip",
  });
  // A separate metrology bench makes the constraint and dimension vocabulary
  // visible without entangling the operation specimens above. One datum point
  // removes only this bench's global gauge; relational constraints establish
  // its two right-angle frames and matched inspection bores.
  const datumBaseline = $.geometry.segment("datumBaseline", {
    start: [-38, -39],
    end: [-24, -39],
    branchDirection: [1, 0],
    role: "construction",
    label: "Primary datum baseline",
  });
  const datumUpright = $.geometry.segment("datumUpright", {
    start: [-38, -39],
    end: [-38, -29],
    branchDirection: [0, 1],
    role: "construction",
    label: "Primary datum upright",
  });
  const followerBaseline = $.geometry.segment("followerBaseline", {
    start: [-16, -39],
    end: [-2, -39],
    branchDirection: [1, 0],
    role: "construction",
    label: "Matched secondary baseline",
  });
  const followerUpright = $.geometry.segment("followerUpright", {
    start: [-16, -39],
    end: [-16, -29],
    branchDirection: [0, 1],
    role: "construction",
    label: "Secondary datum upright",
  });
  const datumMidpoint = $.geometry.sketchPoint("datumMidpoint", {
    point: [-31, -39],
    role: "construction",
    label: "Baseline midpoint witness",
  });
  const inspectionBore = $.geometry.centerRadiusCircle("inspectionBore", {
    center: [24, -34],
    radius: mm(3),
    role: "profile",
    label: "Driven inspection bore",
  });
  const comparisonBore = $.geometry.centerRadiusCircle("comparisonBore", {
    center: [36, -34],
    radius: mm(3),
    role: "profile",
    label: "Equal-radius comparison bore",
  });
  const datumAnchor = $.constraint.fixedPoint("datumAnchor", {
    point: datumBaseline.start,
    target: [-38, -39],
    label: "Primary datum origin",
  });
  const baselineHorizontal = $.constraint.horizontal("baselineHorizontal", {
    span: datumBaseline.span,
    label: "Primary baseline horizontal",
  });
  const uprightVertical = $.constraint.vertical("uprightVertical", {
    span: datumUpright.span,
    label: "Primary upright vertical",
  });
  const datumCorner = $.constraint.coincident("datumCorner", {
    first: datumBaseline.start,
    second: datumUpright.start,
    label: "Primary datum corner",
  });
  const followerHorizontal = $.constraint.horizontal("followerHorizontal", {
    span: followerBaseline.span,
    label: "Secondary baseline horizontal",
  });
  const followerVertical = $.constraint.vertical("followerVertical", {
    span: followerUpright.span,
    label: "Secondary upright vertical",
  });
  const followerCorner = $.constraint.coincident("followerCorner", {
    first: followerBaseline.start,
    second: followerUpright.start,
    label: "Secondary datum corner",
  });
  const matchedBaselines = $.constraint.equalLength("matchedBaselines", {
    first: datumBaseline.span,
    second: followerBaseline.span,
    label: "Matched baseline lengths",
  });
  const midpointWitness = $.constraint.midpoint("midpointWitness", {
    point: datumMidpoint.point,
    line: datumBaseline.span,
    label: "Witness at baseline midpoint",
  });
  const boreCentersAligned = $.constraint.horizontalPoints("boreCentersAligned", {
    first: inspectionBore.center,
    second: comparisonBore.center,
    label: "Inspection bore centres aligned",
  });
  const matchedBoreRadii = $.constraint.equalRadius("matchedBoreRadii", {
    first: inspectionBore.curve,
    second: comparisonBore.curve,
    label: "Inspection bores share radius",
  });
  const baselineLength = $.dimension.curveLength("baselineLength", {
    curve: datumBaseline.span,
    value: mm(14),
    mode: "driving",
    label: "Primary datum length · 14 mm",
  });
  const uprightHeight = $.dimension.curveLength("uprightHeight", {
    curve: datumUpright.span,
    value: mm(10),
    mode: "driving",
    label: "Primary datum height · 10 mm",
  });
  const followerHeight = $.dimension.curveLength("followerHeight", {
    curve: followerUpright.span,
    value: mm(10),
    mode: "driving",
    label: "Secondary datum height · 10 mm",
  });
  const datumAngle = $.dimension.orientedAngle("datumAngle", {
    first: datumBaseline.span,
    second: datumUpright.span,
    value: deg(90),
    orientation: "counterClockwise",
    mode: "reference",
    label: "Reference datum angle · 90°",
  });
  const inspectionDiameter = $.dimension.diameter("inspectionDiameter", {
    curve: inspectionBore.curve,
    value: mm(6),
    mode: "driving",
    label: "Driven inspection diameter · 6 mm",
  });
  const comparisonRadius = $.dimension.radius("comparisonRadius", {
    curve: comparisonBore.curve,
    value: mm(3),
    mode: "reference",
    label: "Reference comparison radius · 3 mm",
  });
  // Driving and reference annotations stay next to the functional specimens
  // they describe, rather than living in generic type-based buckets.
  const seedChord = $.dimension.pointDistance("seedChord", {
    first: splitSource.start,
    second: splitSource.end,
    value: mm(14),
    mode: "reference",
    label: "Reference split-source chord · 14 mm",
  });
  const markerEdge = $.dimension.curveLength("markerEdge", {
    curve: pilotHorizontal.span,
    value: mm(2.8),
    mode: "reference",
    label: "Reference marker edge",
  });
  $.group("Profile generators", [stockBlank, hexFlange, obroundPort]);
  $.group("Split break trim extend", [splitSource, splitAtDatum, breakSource, reliefBreak, trimSource, finishTrim, extensionSource, extensionLimit, extendToLimit]);
  $.group("Mirror chamfer fillet offset", [mirrorAxis, mirrorSeed, reflectedProfile, chamferHorizontal, chamferVertical, cornerChamfer, filletHorizontal, filletVertical, cornerFillet, offsetSource, offsetChain, machiningAllowance]);
  $.group("Generated drilling pattern", [pilotHorizontal, pilotVertical, holeStrip]);
  $.group("Constraint and metrology bench", [datumBaseline, datumUpright, followerBaseline, followerUpright, datumMidpoint, inspectionBore, comparisonBore, datumAnchor, baselineHorizontal, uprightVertical, datumCorner, followerHorizontal, followerVertical, followerCorner, matchedBaselines, midpointWitness, boreCentersAligned, matchedBoreRadii, baselineLength, uprightHeight, followerHeight, datumAngle, inspectionDiameter, comparisonRadius]);
  $.group("Fabrication annotations", [seedChord, markerEdge]);
  return {
    generators: {
      blank: stockBlank,
      flange: hexFlange,
      port: obroundPort,
    },
    topology: {
      split: splitAtDatum,
      break: reliefBreak,
      trim: finishTrim,
      extend: extendToLimit,
    },
    finishing: {
      mirror: reflectedProfile,
      chamfer: cornerChamfer,
      fillet: cornerFillet,
      offset: machiningAllowance,
    },
    pattern: holeStrip,
    metrology: {
      datum: datumBaseline,
      midpoint: datumMidpoint,
      bores: [inspectionBore, comparisonBore],
    },
  };
});
