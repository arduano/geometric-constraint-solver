"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // Plain 1 x 1 x 3U centre section. Profile dimensions follow
  // kennetek/gridfinity-rebuilt-openscad src/core/standard.scad at commit
  // 910e22d8607fd7f5f51ad5e5cbc5287a76810bfd (MIT; see the bundled NOTICE).
  const section = $.geometry.polyline("section", {
    vertices: [{
      key: "baseBottomLeft",
      position: [-17.8, 0],
    }, {
      key: "baseBottomRight",
      position: [17.8, 0],
    }, {
      key: "rightBaseLowerChamferEnd",
      position: [18.6, 0.8],
    }, {
      key: "rightBaseVerticalEnd",
      position: [18.6, 2.6],
    }, {
      key: "rightBaseProfileTop",
      position: [20.75, 4.75],
    }, {
      key: "rightBaseTop",
      position: [20.75, 7],
    }, {
      key: "rightBodyTop",
      position: [20.75, 21],
    }, {
      key: "rightLipCrown",
      position: [20.75, 25.4],
    }, {
      key: "rightLipUpperShoulder",
      position: [18.85, 23.5],
    }, {
      key: "rightLipLowerShoulder",
      position: [18.85, 21.7],
    }, {
      key: "rightLipInnerTip",
      position: [18.15, 21],
    }, {
      key: "rightLipSupportInner",
      position: [18.15, 19.8],
    }, {
      key: "rightLipSupportWall",
      position: [19.8, 18.15],
    }, {
      key: "rightCavityFloorCorner",
      position: [19.8, 7],
    }, {
      key: "leftCavityFloorCorner",
      position: [-19.8, 7],
    }, {
      key: "leftLipSupportWall",
      position: [-19.8, 18.15],
    }, {
      key: "leftLipSupportInner",
      position: [-18.15, 19.8],
    }, {
      key: "leftLipInnerTip",
      position: [-18.15, 21],
    }, {
      key: "leftLipLowerShoulder",
      position: [-18.85, 21.7],
    }, {
      key: "leftLipUpperShoulder",
      position: [-18.85, 23.5],
    }, {
      key: "leftLipCrown",
      position: [-20.75, 25.4],
    }, {
      key: "leftBodyTop",
      position: [-20.75, 21],
    }, {
      key: "leftBaseTop",
      position: [-20.75, 7],
    }, {
      key: "leftBaseProfileTop",
      position: [-20.75, 4.75],
    }, {
      key: "leftBaseVerticalEnd",
      position: [-18.6, 2.6],
    }, {
      key: "leftBaseLowerChamferEnd",
      position: [-18.6, 0.8],
    }],
    closed: true,
    branchDirections: [[1, 0], [0.7071067811865475, 0.7071067811865475], [0, 1], [0.7071067811865475, 0.7071067811865475], [0, 1], [0, 1], [0, 1], [-0.7071067811865475, -0.7071067811865475], [0, -1], [-0.7071067811865475, -0.7071067811865475], [0, -1], [0.7071067811865475, -0.7071067811865475], [0, -1], [-1, 0], [0, 1], [0.7071067811865475, 0.7071067811865475], [0, 1], [-0.7071067811865475, 0.7071067811865475], [0, 1], [-0.7071067811865475, 0.7071067811865475], [0, -1], [0, -1], [0, -1], [0.7071067811865475, -0.7071067811865475], [0, -1], [0.7071067811865475, -0.7071067811865475]],
  });
  // The contour is governed from one scalar datum. Every left/right pair is
  // a live mirror relation; right-side standards then drive the whole profile.
  const sectionBaseYDatum = $.constraint.fixedCoordinate("sectionBaseYDatum", {
    point: section.vertices.byKey.baseBottomLeft,
    axis: "y",
    target: mm(0),
  });
  const baseBottomSymmetry = $.constraint.symmetricAboutDatumAxis("baseBottomSymmetry", {
    first: section.vertices.byKey.baseBottomLeft,
    second: section.vertices.byKey.baseBottomRight,
    axis: "y",
  });
  const baseLowerChamferSymmetry = $.constraint.symmetricAboutDatumAxis("baseLowerChamferSymmetry", {
    first: section.vertices.byKey.leftBaseLowerChamferEnd,
    second: section.vertices.byKey.rightBaseLowerChamferEnd,
    axis: "y",
  });
  const baseVerticalEndSymmetry = $.constraint.symmetricAboutDatumAxis("baseVerticalEndSymmetry", {
    first: section.vertices.byKey.leftBaseVerticalEnd,
    second: section.vertices.byKey.rightBaseVerticalEnd,
    axis: "y",
  });
  const baseProfileTopSymmetry = $.constraint.symmetricAboutDatumAxis("baseProfileTopSymmetry", {
    first: section.vertices.byKey.leftBaseProfileTop,
    second: section.vertices.byKey.rightBaseProfileTop,
    axis: "y",
  });
  const baseTopSymmetry = $.constraint.symmetricAboutDatumAxis("baseTopSymmetry", {
    first: section.vertices.byKey.leftBaseTop,
    second: section.vertices.byKey.rightBaseTop,
    axis: "y",
  });
  const bodyTopSymmetry = $.constraint.symmetricAboutDatumAxis("bodyTopSymmetry", {
    first: section.vertices.byKey.leftBodyTop,
    second: section.vertices.byKey.rightBodyTop,
    axis: "y",
  });
  const lipCrownSymmetry = $.constraint.symmetricAboutDatumAxis("lipCrownSymmetry", {
    first: section.vertices.byKey.leftLipCrown,
    second: section.vertices.byKey.rightLipCrown,
    axis: "y",
  });
  const lipUpperShoulderSymmetry = $.constraint.symmetricAboutDatumAxis("lipUpperShoulderSymmetry", {
    first: section.vertices.byKey.leftLipUpperShoulder,
    second: section.vertices.byKey.rightLipUpperShoulder,
    axis: "y",
  });
  const lipLowerShoulderSymmetry = $.constraint.symmetricAboutDatumAxis("lipLowerShoulderSymmetry", {
    first: section.vertices.byKey.leftLipLowerShoulder,
    second: section.vertices.byKey.rightLipLowerShoulder,
    axis: "y",
  });
  const lipInnerTipSymmetry = $.constraint.symmetricAboutDatumAxis("lipInnerTipSymmetry", {
    first: section.vertices.byKey.leftLipInnerTip,
    second: section.vertices.byKey.rightLipInnerTip,
    axis: "y",
  });
  const lipSupportInnerSymmetry = $.constraint.symmetricAboutDatumAxis("lipSupportInnerSymmetry", {
    first: section.vertices.byKey.leftLipSupportInner,
    second: section.vertices.byKey.rightLipSupportInner,
    axis: "y",
  });
  const lipSupportWallSymmetry = $.constraint.symmetricAboutDatumAxis("lipSupportWallSymmetry", {
    first: section.vertices.byKey.leftLipSupportWall,
    second: section.vertices.byKey.rightLipSupportWall,
    axis: "y",
  });
  const cavityFloorSymmetry = $.constraint.symmetricAboutDatumAxis("cavityFloorSymmetry", {
    first: section.vertices.byKey.leftCavityFloorCorner,
    second: section.vertices.byKey.rightCavityFloorCorner,
    axis: "y",
  });
  const baseBottomWidth = $.dimension.curveLength("baseBottomWidth", {
    curve: section.segments.byKey.baseBottomLeft,
    value: mm(35.6),
  });
  const baseVertical = $.constraint.vertical("baseVertical", {
    span: section.segments.byKey.rightBaseLowerChamferEnd,
  });
  const baseVerticalLength = $.dimension.curveLength("baseVerticalLength", {
    curve: section.segments.byKey.rightBaseLowerChamferEnd,
    value: mm(1.8),
  });
  const baseTopRise = $.constraint.vertical("baseTopRise", {
    span: section.segments.byKey.rightBaseProfileTop,
  });
  const baseTopRiseLength = $.dimension.curveLength("baseTopRiseLength", {
    curve: section.segments.byKey.rightBaseProfileTop,
    value: mm(2.25),
  });
  const bodyRise = $.constraint.vertical("bodyRise", {
    span: section.segments.byKey.rightBaseTop,
  });
  const bodyRiseLength = $.dimension.curveLength("bodyRiseLength", {
    curve: section.segments.byKey.rightBaseTop,
    value: mm(14),
  });
  const lipRise = $.constraint.vertical("lipRise", {
    span: section.segments.byKey.rightBodyTop,
  });
  const lipRiseLength = $.dimension.curveLength("lipRiseLength", {
    curve: section.segments.byKey.rightBodyTop,
    value: mm(4.4),
  });
  const upperShoulderDrop = $.constraint.vertical("upperShoulderDrop", {
    span: section.segments.byKey.rightLipUpperShoulder,
  });
  const upperShoulderDropLength = $.dimension.curveLength("upperShoulderDropLength", {
    curve: section.segments.byKey.rightLipUpperShoulder,
    value: mm(1.8),
  });
  const innerSupportDrop = $.constraint.vertical("innerSupportDrop", {
    span: section.segments.byKey.rightLipInnerTip,
  });
  const innerSupportDropLength = $.dimension.curveLength("innerSupportDropLength", {
    curve: section.segments.byKey.rightLipInnerTip,
    value: mm(1.2),
  });
  const cavityWall = $.constraint.vertical("cavityWall", {
    span: section.segments.byKey.rightLipSupportWall,
  });
  const cavityFloorAtBase = $.constraint.horizontalPoints("cavityFloorAtBase", {
    first: section.vertices.byKey.rightCavityFloorCorner,
    second: section.vertices.byKey.rightBaseTop,
  });
  // Diagonal standard stages use orthogonal construction projections so both
  // components remain intentional and editable instead of frozen coordinates.
  const lowerChamferX = $.geometry.segment("lowerChamferX", {
    start: section.vertices.byKey.baseBottomRight,
    end: [18.6, 0],
    branchDirection: [1, 0],
    role: "construction",
  });
  const lowerChamferY = $.geometry.segment("lowerChamferY", {
    start: lowerChamferX.end,
    end: section.vertices.byKey.rightBaseLowerChamferEnd,
    branchDirection: [0, 1],
    role: "construction",
  });
  const lowerChamferXHorizontal = $.constraint.horizontal("lowerChamferXHorizontal", {
    span: lowerChamferX.span,
  });
  const lowerChamferYVertical = $.constraint.vertical("lowerChamferYVertical", {
    span: lowerChamferY.span,
  });
  const lowerChamferXLength = $.dimension.curveLength("lowerChamferXLength", {
    curve: lowerChamferX.span,
    value: mm(0.8),
  });
  const lowerChamferYLength = $.dimension.curveLength("lowerChamferYLength", {
    curve: lowerChamferY.span,
    value: mm(0.8),
  });
  const baseSlopeX = $.geometry.segment("baseSlopeX", {
    start: section.vertices.byKey.rightBaseVerticalEnd,
    end: [20.75, 2.6],
    branchDirection: [1, 0],
    role: "construction",
  });
  const baseSlopeY = $.geometry.segment("baseSlopeY", {
    start: baseSlopeX.end,
    end: section.vertices.byKey.rightBaseProfileTop,
    branchDirection: [0, 1],
    role: "construction",
  });
  const baseSlopeXHorizontal = $.constraint.horizontal("baseSlopeXHorizontal", {
    span: baseSlopeX.span,
  });
  const baseSlopeYVertical = $.constraint.vertical("baseSlopeYVertical", {
    span: baseSlopeY.span,
  });
  const baseSlopeXLength = $.dimension.curveLength("baseSlopeXLength", {
    curve: baseSlopeX.span,
    value: mm(2.15),
  });
  const baseSlopeYLength = $.dimension.curveLength("baseSlopeYLength", {
    curve: baseSlopeY.span,
    value: mm(2.15),
  });
  const crownSlopeX = $.geometry.segment("crownSlopeX", {
    start: section.vertices.byKey.rightLipCrown,
    end: [18.85, 25.4],
    branchDirection: [-1, 0],
    role: "construction",
  });
  const crownSlopeY = $.geometry.segment("crownSlopeY", {
    start: crownSlopeX.end,
    end: section.vertices.byKey.rightLipUpperShoulder,
    branchDirection: [0, -1],
    role: "construction",
  });
  const crownSlopeXHorizontal = $.constraint.horizontal("crownSlopeXHorizontal", {
    span: crownSlopeX.span,
  });
  const crownSlopeYVertical = $.constraint.vertical("crownSlopeYVertical", {
    span: crownSlopeY.span,
  });
  const crownSlopeXLength = $.dimension.curveLength("crownSlopeXLength", {
    curve: crownSlopeX.span,
    value: mm(1.9),
  });
  const crownSlopeYLength = $.dimension.curveLength("crownSlopeYLength", {
    curve: crownSlopeY.span,
    value: mm(1.9),
  });
  const tipSlopeX = $.geometry.segment("tipSlopeX", {
    start: section.vertices.byKey.rightLipLowerShoulder,
    end: [18.15, 21.7],
    branchDirection: [-1, 0],
    role: "construction",
  });
  const tipSlopeY = $.geometry.segment("tipSlopeY", {
    start: tipSlopeX.end,
    end: section.vertices.byKey.rightLipInnerTip,
    branchDirection: [0, -1],
    role: "construction",
  });
  const tipSlopeXHorizontal = $.constraint.horizontal("tipSlopeXHorizontal", {
    span: tipSlopeX.span,
  });
  const tipSlopeYVertical = $.constraint.vertical("tipSlopeYVertical", {
    span: tipSlopeY.span,
  });
  const tipSlopeXLength = $.dimension.curveLength("tipSlopeXLength", {
    curve: tipSlopeX.span,
    value: mm(0.7),
  });
  const tipSlopeYLength = $.dimension.curveLength("tipSlopeYLength", {
    curve: tipSlopeY.span,
    value: mm(0.7),
  });
  const supportSlopeX = $.geometry.segment("supportSlopeX", {
    start: section.vertices.byKey.rightLipSupportInner,
    end: [19.8, 19.8],
    branchDirection: [1, 0],
    role: "construction",
  });
  const supportSlopeY = $.geometry.segment("supportSlopeY", {
    start: supportSlopeX.end,
    end: section.vertices.byKey.rightLipSupportWall,
    branchDirection: [0, -1],
    role: "construction",
  });
  const supportSlopeXHorizontal = $.constraint.horizontal("supportSlopeXHorizontal", {
    span: supportSlopeX.span,
  });
  const supportSlopeYVertical = $.constraint.vertical("supportSlopeYVertical", {
    span: supportSlopeY.span,
  });
  const supportSlopeXLength = $.dimension.curveLength("supportSlopeXLength", {
    curve: supportSlopeX.span,
    value: mm(1.65),
  });
  const supportSlopeYLength = $.dimension.curveLength("supportSlopeYLength", {
    curve: supportSlopeY.span,
    value: mm(1.65),
  });
  // The plan view is intentionally separate from the 3U centre section so the
  // Explorer can isolate either interpretation of the same 42 mm grid module.
  const plan = $.geometry.twoPointAlignedRectangle("plan", {
    firstCorner: [-20.75, 43],
    oppositeCorner: [20.75, 84.5],
    label: "1 x 1 Gridfinity plan envelope",
    role: "profile",
  });
  const planAnchor = $.constraint.fixedCoordinate("planAnchor", {
    point: plan.corners[0],
    axis: "y",
    target: mm(43),
    label: "Plan baseline",
  });
  const planWidthSymmetry = $.constraint.symmetricAboutDatumAxis("planWidthSymmetry", {
    first: plan.corners[0],
    second: plan.corners[1],
    axis: "y",
  });
  const planMatchesSection = $.constraint.verticalPoints("planMatchesSection", {
    first: plan.corners[1],
    second: section.vertices.byKey.rightBaseTop,
  });
  const planWidth = $.dimension.curveLength("planWidth", {
    curve: plan.spans[0],
    value: mm(41.5),
    label: "Section-linked plan width",
    mode: "reference",
  });
  const planDepth = $.dimension.curveLength("planDepth", {
    curve: plan.spans[1],
    value: mm(41.5),
    label: "Plan depth",
    mode: "driving",
  });
  const planDiagonal = $.geometry.segment("planDiagonal", {
    start: plan.corners[0],
    end: plan.corners[2],
    branchDirection: [0.7071067811865475, 0.7071067811865475],
    label: "Plan-centre diagonal",
    role: "construction",
  });
  const planCenter = $.geometry.sketchPoint("planCenter", {
    point: [0, 63.75],
    label: "Plan centre",
    role: "construction",
  });
  const planCenterAtMidpoint = $.constraint.midpoint("planCenterAtMidpoint", {
    point: planCenter.point,
    line: planDiagonal.span,
    label: "Plan centre",
  });
  // The plan shows the actual cavity footprint, linked to the section wall.
  const planCavity = $.geometry.twoPointAlignedRectangle("planCavity", {
    firstCorner: [-19.8, 43.95],
    oppositeCorner: [19.8, 83.55],
    label: "Plan cavity footprint",
  });
  const planCavityDiagonal = $.geometry.segment("planCavityDiagonal", {
    start: planCavity.corners[0],
    end: planCavity.corners[2],
    branchDirection: [0.7071067811865475, 0.7071067811865475],
    role: "construction",
  });
  const planCavityCentered = $.constraint.midpoint("planCavityCentered", {
    point: planCenter.point,
    line: planCavityDiagonal.span,
  });
  const planCavityWall = $.constraint.verticalPoints("planCavityWall", {
    first: planCavity.corners[1],
    second: section.vertices.byKey.rightCavityFloorCorner,
  });
  const planWallInset = $.geometry.segment("planWallInset", {
    start: plan.corners[0],
    end: [-20.75, 43.95],
    branchDirection: [0, 1],
    role: "construction",
  });
  const planWallInsetVertical = $.constraint.vertical("planWallInsetVertical", {
    span: planWallInset.span,
  });
  const planWallInsetLength = $.dimension.curveLength("planWallInsetLength", {
    curve: planWallInset.span,
    value: mm(0.95),
    label: "Plan wall thickness",
  });
  const planCavityFloorAlignment = $.constraint.horizontalPoints("planCavityFloorAlignment", {
    first: planWallInset.end,
    second: planCavity.corners[0],
  });
  $.group("3U material section", [section]);
  $.group("Section standard dimensions", [sectionBaseYDatum, baseBottomSymmetry, baseLowerChamferSymmetry, baseVerticalEndSymmetry, baseProfileTopSymmetry, baseTopSymmetry, bodyTopSymmetry, lipCrownSymmetry, lipUpperShoulderSymmetry, lipLowerShoulderSymmetry, lipInnerTipSymmetry, lipSupportInnerSymmetry, lipSupportWallSymmetry, cavityFloorSymmetry, baseBottomWidth, baseVertical, baseVerticalLength, baseTopRise, baseTopRiseLength, bodyRise, bodyRiseLength, lipRise, lipRiseLength, upperShoulderDrop, upperShoulderDropLength, innerSupportDrop, innerSupportDropLength, cavityWall, cavityFloorAtBase]);
  $.group("Section projection datums", [lowerChamferX, lowerChamferY, lowerChamferXHorizontal, lowerChamferYVertical, lowerChamferXLength, lowerChamferYLength, baseSlopeX, baseSlopeY, baseSlopeXHorizontal, baseSlopeYVertical, baseSlopeXLength, baseSlopeYLength, crownSlopeX, crownSlopeY, crownSlopeXHorizontal, crownSlopeYVertical, crownSlopeXLength, crownSlopeYLength, tipSlopeX, tipSlopeY, tipSlopeXHorizontal, tipSlopeYVertical, tipSlopeXLength, tipSlopeYLength, supportSlopeX, supportSlopeY, supportSlopeXHorizontal, supportSlopeYVertical, supportSlopeXLength, supportSlopeYLength]);
  $.group("1 x 1 plan study", [plan, planAnchor, planWidthSymmetry, planMatchesSection, planWidth, planDepth, planDiagonal, planCenter, planCenterAtMidpoint, planCavity, planCavityDiagonal, planCavityCentered, planCavityWall, planWallInset, planWallInsetVertical, planWallInsetLength, planCavityFloorAlignment]);
  return {};
});
