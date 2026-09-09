"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
import { waterChannel } from "./patches/water-channel.patch.ts";
import { pointToPointChannel } from "./patches/point-to-point-channel.patch.ts";
import { siliconeGroove } from "./patches/silicone-groove.patch.ts";

export default sketch({
  title: "PC liquid-cooling manifold",
  description: "A constrained acrylic distribution-plate study combines three 12 mm reservoir channels, a separate stair-shaped point-to-point passage, rounded walls, five port bores, one enclosing 2.4 mm silicone seal groove and an external fastener stack.",
}, ($) => {
  // One absolute anchor plus driving dimensions locates the complete 240 × 120 mm plate.
  const plate = $.geometry.twoPointAlignedRectangle("plate", {
    firstCorner: [-120, -60],
    oppositeCorner: [120, 60],
  });
  const plateAnchor = $.constraint.fixedPoint("plateAnchor", {
    point: plate.corners[0],
    target: [-120, -60],
  });
  const plateWidth = $.dimension.curveLength("plateWidth", {
    isKeyConstraint: true,
    curve: plate.spans[0],
    value: mm(240),
  });
  const plateHeight = $.dimension.curveLength("plateHeight", {
    isKeyConstraint: true,
    curve: plate.spans[1],
    value: mm(120),
  });
  // Construction datums place a 60 × 84 mm reservoir bay without coordinate locks.
  const reservoirInsetX = $.geometry.segment("reservoirInsetX", {
    start: plate.corners[0],
    end: [-108, -60],
    branchDirection: [1, 0],
    role: "construction",
  });
  const reservoirInsetXAxis = $.constraint.horizontal("reservoirInsetXAxis", {
    span: reservoirInsetX.span,
  });
  const reservoirInsetXLength = $.dimension.curveLength("reservoirInsetXLength", {
    curve: reservoirInsetX.span,
    value: mm(12),
  });
  const reservoirInsetY = $.geometry.segment("reservoirInsetY", {
    start: reservoirInsetX.end,
    end: [-108, -42],
    branchDirection: [0, 1],
    role: "construction",
  });
  const reservoirInsetYAxis = $.constraint.vertical("reservoirInsetYAxis", {
    span: reservoirInsetY.span,
  });
  const reservoirInsetYLength = $.dimension.curveLength("reservoirInsetYLength", {
    curve: reservoirInsetY.span,
    value: mm(18),
  });
  const reservoir = $.geometry.twoPointAlignedRectangle("reservoir", {
    firstCorner: [-108, -42],
    oppositeCorner: [-48, 42],
    role: "construction",
  });
  const reservoirLocated = $.constraint.coincident("reservoirLocated", {
    first: reservoir.corners[0],
    second: reservoirInsetY.end,
  });
  const reservoirWidth = $.dimension.curveLength("reservoirWidth", {
    isKeyConstraint: true,
    curve: reservoir.spans[0],
    value: mm(60),
  });
  const reservoirHeight = $.dimension.curveLength("reservoirHeight", {
    isKeyConstraint: true,
    curve: reservoir.spans[1],
    value: mm(84),
  });
  // A dimensioned construction spine locates all three reservoir outlets.
  const lowerOutletDatum = $.geometry.segment("lowerOutletDatum", {
    start: reservoir.corners[1],
    end: [-48, -22],
    branchDirection: [0, 1],
    role: "construction",
  });
  const lowerOutletAxis = $.constraint.vertical("lowerOutletAxis", {
    span: lowerOutletDatum.span,
  });
  const lowerOutletLength = $.dimension.curveLength("lowerOutletLength", {
    curve: lowerOutletDatum.span,
    value: mm(20),
  });
  const middleOutletDatum = $.geometry.segment("middleOutletDatum", {
    start: lowerOutletDatum.end,
    end: [-48, 0],
    branchDirection: [0, 1],
    role: "construction",
  });
  const middleOutletAxis = $.constraint.vertical("middleOutletAxis", {
    span: middleOutletDatum.span,
  });
  const middleOutletLength = $.dimension.curveLength("middleOutletLength", {
    curve: middleOutletDatum.span,
    value: mm(22),
  });
  const upperOutletDatum = $.geometry.segment("upperOutletDatum", {
    start: middleOutletDatum.end,
    end: [-48, 22],
    branchDirection: [0, 1],
    role: "construction",
  });
  const upperOutletAxis = $.constraint.vertical("upperOutletAxis", {
    span: upperOutletDatum.span,
  });
  const upperOutletLength = $.dimension.curveLength("upperOutletLength", {
    curve: upperOutletDatum.span,
    value: mm(22),
  });
  // Routed channel centre lines join one shared reservoir.
  const upperCenterline = $.geometry.polyline("upperCenterline", {
    vertices: [{
      key: "reservoir",
      position: [-48, 22],
    }, {
      key: "approach",
      position: [-26, 22],
    }, {
      key: "rise",
      position: [-26, 40],
    }, {
      key: "port",
      position: [96, 40],
    }],
    closed: false,
    branchDirections: [[1, 0], [0, 1], [1, 0]],
    role: "construction",
  });
  const upperCenterLocated = $.constraint.coincident("upperCenterLocated", {
    first: upperCenterline.vertices.byKey.reservoir,
    second: upperOutletDatum.end,
  });
  const upperCenterAxis0 = $.constraint.horizontal("upperCenterAxis0", {
    span: upperCenterline.segments.byKey.reservoir,
  });
  const upperCenterAxis1 = $.constraint.vertical("upperCenterAxis1", {
    span: upperCenterline.segments.byKey.approach,
  });
  const upperCenterAxis2 = $.constraint.horizontal("upperCenterAxis2", {
    span: upperCenterline.segments.byKey.rise,
  });
  const upperCenterLength0 = $.dimension.curveLength("upperCenterLength0", {
    curve: upperCenterline.segments.byKey.reservoir,
    value: mm(22),
  });
  const upperCenterLength1 = $.dimension.curveLength("upperCenterLength1", {
    curve: upperCenterline.segments.byKey.approach,
    value: mm(18),
  });
  const upperCenterLength2 = $.dimension.curveLength("upperCenterLength2", {
    curve: upperCenterline.segments.byKey.rise,
    value: mm(122),
  });
  // Middle route deliberately differs in keyed topology and packaging dimensions.
  const middleCenterline = $.geometry.polyline("middleCenterline", {
    vertices: [{
      key: "reservoir",
      position: [-48, 0],
    }, {
      key: "approach",
      position: [-4, 0],
    }, {
      key: "rise",
      position: [-4, 18],
    }, {
      key: "port",
      position: [96, 18],
    }],
    closed: false,
    branchDirections: [[1, 0], [0, 1], [1, 0]],
    role: "construction",
  });
  const middleCenterLocated = $.constraint.coincident("middleCenterLocated", {
    first: middleCenterline.vertices.byKey.reservoir,
    second: middleOutletDatum.end,
  });
  const middleCenterAxis0 = $.constraint.horizontal("middleCenterAxis0", {
    span: middleCenterline.segments.byKey.reservoir,
  });
  const middleCenterAxis1 = $.constraint.vertical("middleCenterAxis1", {
    span: middleCenterline.segments.byKey.approach,
  });
  const middleCenterAxis2 = $.constraint.horizontal("middleCenterAxis2", {
    span: middleCenterline.segments.byKey.rise,
  });
  const middleCenterLength0 = $.dimension.curveLength("middleCenterLength0", {
    curve: middleCenterline.segments.byKey.reservoir,
    value: mm(44),
  });
  const middleCenterLength1 = $.dimension.curveLength("middleCenterLength1", {
    curve: middleCenterline.segments.byKey.approach,
    value: mm(18),
  });
  const middleCenterLength2 = $.dimension.curveLength("middleCenterLength2", {
    curve: middleCenterline.segments.byKey.rise,
    value: mm(100),
  });
  // Lower route mirrors the packaging direction while keeping its own stable keys.
  const lowerCenterline = $.geometry.polyline("lowerCenterline", {
    vertices: [{
      key: "reservoir",
      position: [-48, -22],
    }, {
      key: "approach",
      position: [-26, -22],
    }, {
      key: "drop",
      position: [-26, -40],
    }, {
      key: "port",
      position: [96, -40],
    }],
    closed: false,
    branchDirections: [[1, 0], [0, -1], [1, 0]],
    role: "construction",
  });
  const lowerCenterLocated = $.constraint.coincident("lowerCenterLocated", {
    first: lowerCenterline.vertices.byKey.reservoir,
    second: lowerOutletDatum.end,
  });
  const lowerCenterAxis0 = $.constraint.horizontal("lowerCenterAxis0", {
    span: lowerCenterline.segments.byKey.reservoir,
  });
  const lowerCenterAxis1 = $.constraint.vertical("lowerCenterAxis1", {
    span: lowerCenterline.segments.byKey.approach,
  });
  const lowerCenterAxis2 = $.constraint.horizontal("lowerCenterAxis2", {
    span: lowerCenterline.segments.byKey.drop,
  });
  const lowerCenterLength0 = $.dimension.curveLength("lowerCenterLength0", {
    curve: lowerCenterline.segments.byKey.reservoir,
    value: mm(22),
  });
  const lowerCenterLength1 = $.dimension.curveLength("lowerCenterLength1", {
    curve: lowerCenterline.segments.byKey.approach,
    value: mm(18),
  });
  const lowerCenterLength2 = $.dimension.curveLength("lowerCenterLength2", {
    curve: lowerCenterline.segments.byKey.drop,
    value: mm(122),
  });
  // Each route is 12 mm wide, with tangent wall bends and a rounded outlet.
  // The open inlets join the reservoir perimeter without closing off the mouths.
  const channelWidth = $.parameter("channelWidth", mm(12), {
    label: "Channel width",
    description: "Full passage width, shared by all four channels.",
    isKeyParameter: true,
  });
  const channelBendRadius = mm(8);
  const upperChannel = $.use("upperChannel", waterChannel, {
    polyline: upperCenterline,
    width: channelWidth,
    bendRadius: channelBendRadius,
  });
  const middleChannel = $.use("middleChannel", waterChannel, {
    polyline: middleCenterline,
    width: channelWidth,
    bendRadius: channelBendRadius,
  });
  const lowerChannel = $.use("lowerChannel", waterChannel, {
    polyline: lowerCenterline,
    width: channelWidth,
    bendRadius: channelBendRadius,
  });
  // A separate stair-shaped passage links two ports in the open lower bay.
  const stairStartX = $.geometry.segment("stairStartX", {
    start: reservoir.corners[1],
    end: [0, -42],
    branchDirection: [1, 0],
    role: "construction",
  });
  const stairStartXAxis = $.constraint.horizontal("stairStartXAxis", {
    span: stairStartX.span,
  });
  const stairStartXLength = $.dimension.curveLength("stairStartXLength", {
    curve: stairStartX.span,
    value: mm(48),
  });
  const stairStartY = $.geometry.segment("stairStartY", {
    start: stairStartX.end,
    end: [0, -20],
    branchDirection: [0, 1],
    role: "construction",
  });
  const stairStartYAxis = $.constraint.vertical("stairStartYAxis", {
    span: stairStartY.span,
  });
  const stairStartYLength = $.dimension.curveLength("stairStartYLength", {
    curve: stairStartY.span,
    value: mm(22),
  });
  const stairCenterline = $.geometry.polyline("stairCenterline", {
    vertices: [{
      key: "inlet",
      position: [0, -20],
    }, {
      key: "approach",
      position: [30, -20],
    }, {
      key: "rise",
      position: [30, -2],
    }, {
      key: "outlet",
      position: [82, -2],
    }],
    closed: false,
    branchDirections: [[1, 0], [0, 1], [1, 0]],
    role: "construction",
  });
  const stairCenterLocated = $.constraint.coincident("stairCenterLocated", {
    first: stairCenterline.vertices.byKey.inlet,
    second: stairStartY.end,
  });
  const stairCenterAxis0 = $.constraint.horizontal("stairCenterAxis0", {
    span: stairCenterline.segments.byKey.inlet,
  });
  const stairCenterAxis1 = $.constraint.vertical("stairCenterAxis1", {
    span: stairCenterline.segments.byKey.approach,
  });
  const stairCenterAxis2 = $.constraint.horizontal("stairCenterAxis2", {
    span: stairCenterline.segments.byKey.rise,
  });
  const stairCenterLength0 = $.dimension.curveLength("stairCenterLength0", {
    curve: stairCenterline.segments.byKey.inlet,
    value: mm(30),
  });
  const stairCenterLength1 = $.dimension.curveLength("stairCenterLength1", {
    curve: stairCenterline.segments.byKey.approach,
    value: mm(18),
  });
  const stairCenterLength2 = $.dimension.curveLength("stairCenterLength2", {
    curve: stairCenterline.segments.byKey.rise,
    value: mm(52),
  });
  const stairChannel = $.use("stairChannel", pointToPointChannel, {
    polyline: stairCenterline,
    width: channelWidth,
    bendRadius: channelBendRadius,
  });
  const stairInlet = $.geometry.centerRadiusCircle("stairInlet", {
    center: stairCenterline.vertices.byKey.inlet,
    radius: mm(3),
    label: "Stair inlet bore",
  });
  const stairInletRadius = $.dimension.radius("stairInletRadius", {
    curve: stairInlet.curve,
    value: mm(3),
  });
  const stairOutlet = $.geometry.centerRadiusCircle("stairOutlet", {
    center: stairCenterline.vertices.byKey.outlet,
    radius: mm(3),
    label: "Stair outlet bore",
  });
  const stairOutletRadius = $.dimension.radius("stairOutletRadius", {
    curve: stairOutlet.curve,
    value: mm(3),
  });
  const reservoirBottom = $.geometry.segment("reservoirBottom", {
    start: reservoir.corners[0],
    end: reservoir.corners[1],
    branchDirection: [1, 0],
  });
  const reservoirLowerMouth = $.geometry.segment("reservoirLowerMouth", {
    start: reservoir.corners[1],
    end: lowerChannel.profile.startRight,
    branchDirection: [0, 1],
  });
  const reservoirLowerBridge = $.geometry.segment("reservoirLowerBridge", {
    start: lowerChannel.profile.startLeft,
    end: middleChannel.profile.startRight,
    branchDirection: [0, 1],
  });
  const reservoirUpperBridge = $.geometry.segment("reservoirUpperBridge", {
    start: middleChannel.profile.startLeft,
    end: upperChannel.profile.startRight,
    branchDirection: [0, 1],
  });
  const reservoirUpperMouth = $.geometry.segment("reservoirUpperMouth", {
    start: upperChannel.profile.startLeft,
    end: reservoir.corners[2],
    branchDirection: [0, 1],
  });
  const reservoirTop = $.geometry.segment("reservoirTop", {
    start: reservoir.corners[2],
    end: reservoir.corners[3],
    branchDirection: [-1, 0],
  });
  const reservoirLeft = $.geometry.segment("reservoirLeft", {
    start: reservoir.corners[3],
    end: reservoir.corners[0],
    branchDirection: [0, -1],
  });
  const sealInsetX = $.geometry.segment("sealInsetX", {
    start: reservoir.corners[0],
    end: [-114, -42],
    branchDirection: [-1, 0],
    role: "construction",
  });
  const sealInsetXAxis = $.constraint.horizontal("sealInsetXAxis", {
    span: sealInsetX.span,
  });
  const sealInsetXLength = $.dimension.curveLength("sealInsetXLength", {
    curve: sealInsetX.span,
    value: mm(6),
  });
  const sealInsetY = $.geometry.segment("sealInsetY", {
    start: sealInsetX.end,
    end: [-114, -50],
    branchDirection: [0, -1],
    role: "construction",
  });
  const sealInsetYAxis = $.constraint.vertical("sealInsetYAxis", {
    span: sealInsetY.span,
  });
  const sealInsetYLength = $.dimension.curveLength("sealInsetYLength", {
    curve: sealInsetY.span,
    value: mm(8),
  });
  const sealPortX = $.geometry.segment("sealPortX", {
    start: upperCenterline.vertices.byKey.port,
    end: [106, 40],
    branchDirection: [1, 0],
    role: "construction",
  });
  const sealPortXAxis = $.constraint.horizontal("sealPortXAxis", {
    span: sealPortX.span,
  });
  const sealPortXLength = $.dimension.curveLength("sealPortXLength", {
    curve: sealPortX.span,
    value: mm(10),
  });
  const sealPortY = $.geometry.segment("sealPortY", {
    start: sealPortX.end,
    end: [106, 50],
    branchDirection: [0, 1],
    role: "construction",
  });
  const sealPortYAxis = $.constraint.vertical("sealPortYAxis", {
    span: sealPortY.span,
  });
  const sealPortYLength = $.dimension.curveLength("sealPortYLength", {
    curve: sealPortY.span,
    value: mm(10),
  });
  // One 2.4 mm wide seal groove encloses the entire connected wet circuit.
  const commonSeal = $.geometry.polyline("commonSeal", {
    vertices: [{
      key: "southWest",
      position: sealInsetY.end,
    }, {
      key: "southEast",
      position: [106, -50],
    }, {
      key: "northEast",
      position: sealPortY.end,
    }, {
      key: "northWest",
      position: [-114, 50],
    }],
    closed: true,
    branchDirections: [[1, 0], [0, 1], [-1, 0], [0, -1]],
    label: "Shared wet-circuit O-ring centreline",
    role: "construction",
  });
  const sealBottomAxis = $.constraint.horizontal("sealBottomAxis", {
    span: commonSeal.segments.byKey.southWest,
  });
  const sealRightAxis = $.constraint.vertical("sealRightAxis", {
    span: commonSeal.segments.byKey.southEast,
  });
  const sealTopAxis = $.constraint.horizontal("sealTopAxis", {
    span: commonSeal.segments.byKey.northEast,
  });
  const sealLeftAxis = $.constraint.vertical("sealLeftAxis", {
    span: commonSeal.segments.byKey.northWest,
  });
  const sealGrooveWidth = $.parameter("sealGrooveWidth", mm(2.4), {
    label: "Seal groove width",
    description: "Full width of the enclosing silicone seal groove.",
    isKeyParameter: true,
  });
  const commonSealGroove = $.use("commonSealGroove", siliconeGroove, {
    polyline: commonSeal,
    width: sealGrooveWidth,
    bendRadius: mm(5),
  });
  // Through-bores share the rounded outlet centres and retain independent radii.
  const upperOutlet = $.geometry.centerRadiusCircle("upperOutlet", {
    center: upperCenterline.vertices.byKey.port,
    radius: mm(3),
    label: "Upper outlet bore",
  });
  const upperOutletRadius = $.dimension.radius("upperOutletRadius", {
    isKeyConstraint: true,
    curve: upperOutlet.curve,
    value: mm(3),
    label: "Outlet radius",
  });
  const middleOutlet = $.geometry.centerRadiusCircle("middleOutlet", {
    center: middleCenterline.vertices.byKey.port,
    radius: mm(3),
    label: "Middle outlet bore",
  });
  const middleOutletRadius = $.dimension.radius("middleOutletRadius", {
    curve: middleOutlet.curve,
    value: mm(3),
    label: "Outlet radius",
  });
  const lowerOutlet = $.geometry.centerRadiusCircle("lowerOutlet", {
    center: lowerCenterline.vertices.byKey.port,
    radius: mm(3),
    label: "Lower outlet bore",
  });
  const lowerOutletRadius = $.dimension.radius("lowerOutletRadius", {
    curve: lowerOutlet.curve,
    value: mm(3),
    label: "Outlet radius",
  });
  // Dimensioned construction rails carry eight referenced 5 mm screw centers.
  const topScrewInset = $.geometry.segment("topScrewInset", {
    start: plate.corners[3],
    end: [-120, 56],
    branchDirection: [0, -1],
    role: "construction",
  });
  const topScrewInsetAxis = $.constraint.vertical("topScrewInsetAxis", {
    span: topScrewInset.span,
  });
  const topScrewInsetLength = $.dimension.curveLength("topScrewInsetLength", {
    curve: topScrewInset.span,
    value: mm(4),
  });
  const topScrewRail0 = $.geometry.segment("topScrewRail0", {
    start: topScrewInset.end,
    end: [-108, 56],
    branchDirection: [1, 0],
    role: "construction",
  });
  const topScrewRail0Axis = $.constraint.horizontal("topScrewRail0Axis", {
    span: topScrewRail0.span,
  });
  const topScrewRail0Length = $.dimension.curveLength("topScrewRail0Length", {
    curve: topScrewRail0.span,
    value: mm(12),
  });
  const topScrewRail1 = $.geometry.segment("topScrewRail1", {
    start: topScrewRail0.end,
    end: [-80, 56],
    branchDirection: [1, 0],
    role: "construction",
  });
  const topScrewRail1Axis = $.constraint.horizontal("topScrewRail1Axis", {
    span: topScrewRail1.span,
  });
  const topScrewRail1Length = $.dimension.curveLength("topScrewRail1Length", {
    curve: topScrewRail1.span,
    value: mm(28),
  });
  const topScrewRail2 = $.geometry.segment("topScrewRail2", {
    start: topScrewRail1.end,
    end: [96, 56],
    branchDirection: [1, 0],
    role: "construction",
  });
  const topScrewRail2Axis = $.constraint.horizontal("topScrewRail2Axis", {
    span: topScrewRail2.span,
  });
  const topScrewRail2Length = $.dimension.curveLength("topScrewRail2Length", {
    curve: topScrewRail2.span,
    value: mm(176),
  });
  const topScrewRail3 = $.geometry.segment("topScrewRail3", {
    start: topScrewRail2.end,
    end: [112, 56],
    branchDirection: [1, 0],
    role: "construction",
  });
  const topScrewRail3Axis = $.constraint.horizontal("topScrewRail3Axis", {
    span: topScrewRail3.span,
  });
  const topScrewRail3Length = $.dimension.curveLength("topScrewRail3Length", {
    curve: topScrewRail3.span,
    value: mm(16),
  });
  const bottomScrewInset = $.geometry.segment("bottomScrewInset", {
    start: plate.corners[0],
    end: [-120, -56],
    branchDirection: [0, 1],
    role: "construction",
  });
  const bottomScrewInsetAxis = $.constraint.vertical("bottomScrewInsetAxis", {
    span: bottomScrewInset.span,
  });
  const bottomScrewInsetLength = $.dimension.curveLength("bottomScrewInsetLength", {
    curve: bottomScrewInset.span,
    value: mm(4),
  });
  const bottomScrewRail0 = $.geometry.segment("bottomScrewRail0", {
    start: bottomScrewInset.end,
    end: [-108, -56],
    branchDirection: [1, 0],
    role: "construction",
  });
  const bottomScrewRail0Axis = $.constraint.horizontal("bottomScrewRail0Axis", {
    span: bottomScrewRail0.span,
  });
  const bottomScrewRail0Length = $.dimension.curveLength("bottomScrewRail0Length", {
    curve: bottomScrewRail0.span,
    value: mm(12),
  });
  const bottomScrewRail1 = $.geometry.segment("bottomScrewRail1", {
    start: bottomScrewRail0.end,
    end: [-80, -56],
    branchDirection: [1, 0],
    role: "construction",
  });
  const bottomScrewRail1Axis = $.constraint.horizontal("bottomScrewRail1Axis", {
    span: bottomScrewRail1.span,
  });
  const bottomScrewRail1Length = $.dimension.curveLength("bottomScrewRail1Length", {
    curve: bottomScrewRail1.span,
    value: mm(28),
  });
  const bottomScrewRail2 = $.geometry.segment("bottomScrewRail2", {
    start: bottomScrewRail1.end,
    end: [96, -56],
    branchDirection: [1, 0],
    role: "construction",
  });
  const bottomScrewRail2Axis = $.constraint.horizontal("bottomScrewRail2Axis", {
    span: bottomScrewRail2.span,
  });
  const bottomScrewRail2Length = $.dimension.curveLength("bottomScrewRail2Length", {
    curve: bottomScrewRail2.span,
    value: mm(176),
  });
  const bottomScrewRail3 = $.geometry.segment("bottomScrewRail3", {
    start: bottomScrewRail2.end,
    end: [112, -56],
    branchDirection: [1, 0],
    role: "construction",
  });
  const bottomScrewRail3Axis = $.constraint.horizontal("bottomScrewRail3Axis", {
    span: bottomScrewRail3.span,
  });
  const bottomScrewRail3Length = $.dimension.curveLength("bottomScrewRail3Length", {
    curve: bottomScrewRail3.span,
    value: mm(16),
  });
  const screwNwOuter = $.geometry.centerRadiusCircle("screwNwOuter", {
    center: topScrewRail0.end,
    radius: mm(2.5),
  });
  const screwNwInner = $.geometry.centerRadiusCircle("screwNwInner", {
    center: topScrewRail1.end,
    radius: mm(2.5),
  });
  const screwNeInner = $.geometry.centerRadiusCircle("screwNeInner", {
    center: topScrewRail2.end,
    radius: mm(2.5),
  });
  const screwNeOuter = $.geometry.centerRadiusCircle("screwNeOuter", {
    center: topScrewRail3.end,
    radius: mm(2.5),
  });
  const screwSwOuter = $.geometry.centerRadiusCircle("screwSwOuter", {
    center: bottomScrewRail0.end,
    radius: mm(2.5),
  });
  const screwSwInner = $.geometry.centerRadiusCircle("screwSwInner", {
    center: bottomScrewRail1.end,
    radius: mm(2.5),
  });
  const screwSeInner = $.geometry.centerRadiusCircle("screwSeInner", {
    center: bottomScrewRail2.end,
    radius: mm(2.5),
  });
  const screwSeOuter = $.geometry.centerRadiusCircle("screwSeOuter", {
    center: bottomScrewRail3.end,
    radius: mm(2.5),
  });
  const screwNwOuterDiameter = $.dimension.diameter("screwNwOuterDiameter", {
    isKeyConstraint: true,
    curve: screwNwOuter.curve,
    value: mm(5),
  });
  const screwNwInnerDiameter = $.dimension.diameter("screwNwInnerDiameter", {
    curve: screwNwInner.curve,
    value: mm(5),
  });
  const screwNeInnerDiameter = $.dimension.diameter("screwNeInnerDiameter", {
    curve: screwNeInner.curve,
    value: mm(5),
  });
  const screwNeOuterDiameter = $.dimension.diameter("screwNeOuterDiameter", {
    curve: screwNeOuter.curve,
    value: mm(5),
  });
  const screwSwOuterDiameter = $.dimension.diameter("screwSwOuterDiameter", {
    curve: screwSwOuter.curve,
    value: mm(5),
  });
  const screwSwInnerDiameter = $.dimension.diameter("screwSwInnerDiameter", {
    curve: screwSwInner.curve,
    value: mm(5),
  });
  const screwSeInnerDiameter = $.dimension.diameter("screwSeInnerDiameter", {
    curve: screwSeInner.curve,
    value: mm(5),
  });
  const screwSeOuterDiameter = $.dimension.diameter("screwSeOuterDiameter", {
    curve: screwSeOuter.curve,
    value: mm(5),
  });
  $.group("Manifold envelope and reservoir", [plate, plateAnchor, plateWidth, plateHeight, reservoirInsetX, reservoirInsetXAxis, reservoirInsetXLength, reservoirInsetY, reservoirInsetYAxis, reservoirInsetYLength, reservoir, reservoirLocated, reservoirWidth, reservoirHeight, lowerOutletDatum, lowerOutletAxis, lowerOutletLength, middleOutletDatum, middleOutletAxis, middleOutletLength, upperOutletDatum, upperOutletAxis, upperOutletLength, reservoirBottom, reservoirLowerMouth, reservoirLowerBridge, reservoirUpperBridge, reservoirUpperMouth, reservoirTop, reservoirLeft]);
  $.group("Upper channel circuit", [upperCenterline, upperCenterLocated, upperCenterAxis0, upperCenterAxis1, upperCenterAxis2, upperCenterLength0, upperCenterLength1, upperCenterLength2, upperChannel, upperOutlet, upperOutletRadius]);
  $.group("Middle channel circuit", [middleCenterline, middleCenterLocated, middleCenterAxis0, middleCenterAxis1, middleCenterAxis2, middleCenterLength0, middleCenterLength1, middleCenterLength2, middleChannel, middleOutlet, middleOutletRadius]);
  $.group("Lower channel circuit", [lowerCenterline, lowerCenterLocated, lowerCenterAxis0, lowerCenterAxis1, lowerCenterAxis2, lowerCenterLength0, lowerCenterLength1, lowerCenterLength2, lowerChannel, lowerOutlet, lowerOutletRadius]);
  $.group("Point-to-point stair channel", [stairStartX, stairStartXAxis, stairStartXLength, stairStartY, stairStartYAxis, stairStartYLength, stairCenterline, stairCenterLocated, stairCenterAxis0, stairCenterAxis1, stairCenterAxis2, stairCenterLength0, stairCenterLength1, stairCenterLength2, stairChannel, stairInlet, stairInletRadius, stairOutlet, stairOutletRadius]);
  $.group("Shared circuit seal", [commonSeal, sealBottomAxis, sealRightAxis, sealTopAxis, sealLeftAxis, commonSealGroove, sealInsetX, sealInsetXAxis, sealInsetXLength, sealInsetY, sealInsetYAxis, sealInsetYLength, sealPortX, sealPortXAxis, sealPortXLength, sealPortY, sealPortYAxis, sealPortYLength]);
  $.group("Fastener stack", [topScrewInset, topScrewInsetAxis, topScrewInsetLength, topScrewRail0, topScrewRail0Axis, topScrewRail0Length, topScrewRail1, topScrewRail1Axis, topScrewRail1Length, topScrewRail2, topScrewRail2Axis, topScrewRail2Length, topScrewRail3, topScrewRail3Axis, topScrewRail3Length, bottomScrewInset, bottomScrewInsetAxis, bottomScrewInsetLength, bottomScrewRail0, bottomScrewRail0Axis, bottomScrewRail0Length, bottomScrewRail1, bottomScrewRail1Axis, bottomScrewRail1Length, bottomScrewRail2, bottomScrewRail2Axis, bottomScrewRail2Length, bottomScrewRail3, bottomScrewRail3Axis, bottomScrewRail3Length, screwNwOuter, screwNwInner, screwNeInner, screwNeOuter, screwSwOuter, screwSwInner, screwSeInner, screwSeOuter, screwNwOuterDiameter, screwNwInnerDiameter, screwNeInnerDiameter, screwNeOuterDiameter, screwSwOuterDiameter, screwSwInnerDiameter, screwSeInnerDiameter, screwSeOuterDiameter]);
  return {};
});
