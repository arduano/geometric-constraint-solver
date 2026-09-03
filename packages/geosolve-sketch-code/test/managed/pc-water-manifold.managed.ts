// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
import { waterChannel } from "./patches/water-channel.patch.ts";

export default sketch(($) => {
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
    curve: plate.spans[0],
    value: mm(240),
  });
  const plateHeight = $.dimension.curveLength("plateHeight", {
    curve: plate.spans[1],
    value: mm(120),
  });

  // Construction datums place a 60 × 84 mm reservoir bay without coordinate locks.
  const reservoirInsetX = $.geometry.segment("reservoirInsetX", {
    start: plate.corners[0],
    end: [-108, -60],
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
  });
  const reservoirLocated = $.constraint.coincident("reservoirLocated", {
    first: reservoir.corners[0],
    second: reservoirInsetY.end,
  });
  const reservoirWidth = $.dimension.curveLength("reservoirWidth", {
    curve: reservoir.spans[0],
    value: mm(60),
  });
  const reservoirHeight = $.dimension.curveLength("reservoirHeight", {
    curve: reservoir.spans[1],
    value: mm(84),
  });

  // A dimensioned construction spine locates all three reservoir outlets.
  const lowerOutletDatum = $.geometry.segment("lowerOutletDatum", {
    start: reservoir.corners[1],
    end: [-48, -30],
    role: "construction",
  });
  const lowerOutletAxis = $.constraint.vertical("lowerOutletAxis", {
    span: lowerOutletDatum.span,
  });
  const lowerOutletLength = $.dimension.curveLength("lowerOutletLength", {
    curve: lowerOutletDatum.span,
    value: mm(12),
  });
  const middleOutletDatum = $.geometry.segment("middleOutletDatum", {
    start: lowerOutletDatum.end,
    end: [-48, 0],
    role: "construction",
  });
  const middleOutletAxis = $.constraint.vertical("middleOutletAxis", {
    span: middleOutletDatum.span,
  });
  const middleOutletLength = $.dimension.curveLength("middleOutletLength", {
    curve: middleOutletDatum.span,
    value: mm(30),
  });
  const upperOutletDatum = $.geometry.segment("upperOutletDatum", {
    start: middleOutletDatum.end,
    end: [-48, 30],
    role: "construction",
  });
  const upperOutletAxis = $.constraint.vertical("upperOutletAxis", {
    span: upperOutletDatum.span,
  });
  const upperOutletLength = $.dimension.curveLength("upperOutletLength", {
    curve: upperOutletDatum.span,
    value: mm(30),
  });

  // Upper water route and a separately constrained closed O-ring groove.
  const upperCenterline = $.geometry.polyline("upperCenterline", {
    vertices: [
      { key: "reservoir", position: [-48, 30] },
      { key: "approach", position: [-26, 30] },
      { key: "rise", position: [-26, 44] },
      { key: "port", position: [100, 44] },
    ],
    closed: false,
  });
  const upperCenterLocated = $.constraint.coincident("upperCenterLocated", {
    first: upperCenterline.vertices.byKey.reservoir,
    second: upperOutletDatum.end,
  });
  const upperCenterAxis0 = $.constraint.horizontal("upperCenterAxis0", { span: upperCenterline.segments.byKey.reservoir });
  const upperCenterAxis1 = $.constraint.vertical("upperCenterAxis1", { span: upperCenterline.segments.byKey.approach });
  const upperCenterAxis2 = $.constraint.horizontal("upperCenterAxis2", { span: upperCenterline.segments.byKey.rise });
  const upperCenterLength0 = $.dimension.curveLength("upperCenterLength0", { curve: upperCenterline.segments.byKey.reservoir, value: mm(22) });
  const upperCenterLength1 = $.dimension.curveLength("upperCenterLength1", { curve: upperCenterline.segments.byKey.approach, value: mm(14) });
  const upperCenterLength2 = $.dimension.curveLength("upperCenterLength2", { curve: upperCenterline.segments.byKey.rise, value: mm(126) });

  const upperSealInsetX = $.geometry.segment("upperSealInsetX", {
    start: upperCenterline.vertices.byKey.reservoir,
    end: [-52, 30],
    role: "construction",
  });
  const upperSealInsetXAxis = $.constraint.horizontal("upperSealInsetXAxis", { span: upperSealInsetX.span });
  const upperSealInsetXLength = $.dimension.curveLength("upperSealInsetXLength", { curve: upperSealInsetX.span, value: mm(4) });
  const upperSealInsetY = $.geometry.segment("upperSealInsetY", {
    start: upperSealInsetX.end,
    end: [-52, 24],
    role: "construction",
  });
  const upperSealInsetYAxis = $.constraint.vertical("upperSealInsetYAxis", { span: upperSealInsetY.span });
  const upperSealInsetYLength = $.dimension.curveLength("upperSealInsetYLength", { curve: upperSealInsetY.span, value: mm(6) });
  const upperSeal = $.geometry.polyline("upperSeal", {
    vertices: [
      { key: "innerStart", position: [-52, 24] },
      { key: "innerRun", position: [106, 24] },
      { key: "outerPort", position: [106, 50] },
      { key: "outerStart", position: [-52, 50] },
    ],
    closed: true,
  });
  const upperSealLocated = $.constraint.coincident("upperSealLocated", {
    first: upperSeal.vertices.byKey.innerStart,
    second: upperSealInsetY.end,
  });
  const upperSealAxis0 = $.constraint.horizontal("upperSealAxis0", { span: upperSeal.segments.byKey.innerStart });
  const upperSealAxis1 = $.constraint.vertical("upperSealAxis1", { span: upperSeal.segments.byKey.innerRun });
  const upperSealAxis2 = $.constraint.horizontal("upperSealAxis2", { span: upperSeal.segments.byKey.outerPort });
  const upperSealAxis3 = $.constraint.vertical("upperSealAxis3", { span: upperSeal.segments.byKey.outerStart });
  const upperSealLength0 = $.dimension.curveLength("upperSealLength0", { curve: upperSeal.segments.byKey.innerStart, value: mm(158) });
  const upperSealLength1 = $.dimension.curveLength("upperSealLength1", { curve: upperSeal.segments.byKey.innerRun, value: mm(26) });

  // Middle route deliberately differs in keyed topology and packaging dimensions.
  const middleCenterline = $.geometry.polyline("middleCenterline", {
    vertices: [
      { key: "reservoir", position: [-48, 0] },
      { key: "approach", position: [-4, 0] },
      { key: "rise", position: [-4, 14] },
      { key: "port", position: [100, 14] },
    ],
    closed: false,
  });
  const middleCenterLocated = $.constraint.coincident("middleCenterLocated", {
    first: middleCenterline.vertices.byKey.reservoir,
    second: middleOutletDatum.end,
  });
  const middleCenterAxis0 = $.constraint.horizontal("middleCenterAxis0", { span: middleCenterline.segments.byKey.reservoir });
  const middleCenterAxis1 = $.constraint.vertical("middleCenterAxis1", { span: middleCenterline.segments.byKey.approach });
  const middleCenterAxis2 = $.constraint.horizontal("middleCenterAxis2", { span: middleCenterline.segments.byKey.rise });
  const middleCenterLength0 = $.dimension.curveLength("middleCenterLength0", { curve: middleCenterline.segments.byKey.reservoir, value: mm(44) });
  const middleCenterLength1 = $.dimension.curveLength("middleCenterLength1", { curve: middleCenterline.segments.byKey.approach, value: mm(14) });
  const middleCenterLength2 = $.dimension.curveLength("middleCenterLength2", { curve: middleCenterline.segments.byKey.rise, value: mm(104) });

  const middleSealInsetX = $.geometry.segment("middleSealInsetX", {
    start: middleCenterline.vertices.byKey.reservoir,
    end: [-52, 0],
    role: "construction",
  });
  const middleSealInsetXAxis = $.constraint.horizontal("middleSealInsetXAxis", { span: middleSealInsetX.span });
  const middleSealInsetXLength = $.dimension.curveLength("middleSealInsetXLength", { curve: middleSealInsetX.span, value: mm(4) });
  const middleSealInsetY = $.geometry.segment("middleSealInsetY", {
    start: middleSealInsetX.end,
    end: [-52, -6],
    role: "construction",
  });
  const middleSealInsetYAxis = $.constraint.vertical("middleSealInsetYAxis", { span: middleSealInsetY.span });
  const middleSealInsetYLength = $.dimension.curveLength("middleSealInsetYLength", { curve: middleSealInsetY.span, value: mm(6) });
  const middleSeal = $.geometry.polyline("middleSeal", {
    vertices: [
      { key: "innerStart", position: [-52, -6] },
      { key: "innerRun", position: [106, -6] },
      { key: "outerPort", position: [106, 20] },
      { key: "outerStart", position: [-52, 20] },
    ],
    closed: true,
  });
  const middleSealLocated = $.constraint.coincident("middleSealLocated", {
    first: middleSeal.vertices.byKey.innerStart,
    second: middleSealInsetY.end,
  });
  const middleSealAxis0 = $.constraint.horizontal("middleSealAxis0", { span: middleSeal.segments.byKey.innerStart });
  const middleSealAxis1 = $.constraint.vertical("middleSealAxis1", { span: middleSeal.segments.byKey.innerRun });
  const middleSealAxis2 = $.constraint.horizontal("middleSealAxis2", { span: middleSeal.segments.byKey.outerPort });
  const middleSealAxis3 = $.constraint.vertical("middleSealAxis3", { span: middleSeal.segments.byKey.outerStart });
  const middleSealLength0 = $.dimension.curveLength("middleSealLength0", { curve: middleSeal.segments.byKey.innerStart, value: mm(158) });
  const middleSealLength1 = $.dimension.curveLength("middleSealLength1", { curve: middleSeal.segments.byKey.innerRun, value: mm(26) });

  // Lower route mirrors the packaging direction while keeping its own stable keys.
  const lowerCenterline = $.geometry.polyline("lowerCenterline", {
    vertices: [
      { key: "reservoir", position: [-48, -30] },
      { key: "approach", position: [-26, -30] },
      { key: "drop", position: [-26, -44] },
      { key: "port", position: [100, -44] },
    ],
    closed: false,
  });
  const lowerCenterLocated = $.constraint.coincident("lowerCenterLocated", {
    first: lowerCenterline.vertices.byKey.reservoir,
    second: lowerOutletDatum.end,
  });
  const lowerCenterAxis0 = $.constraint.horizontal("lowerCenterAxis0", { span: lowerCenterline.segments.byKey.reservoir });
  const lowerCenterAxis1 = $.constraint.vertical("lowerCenterAxis1", { span: lowerCenterline.segments.byKey.approach });
  const lowerCenterAxis2 = $.constraint.horizontal("lowerCenterAxis2", { span: lowerCenterline.segments.byKey.drop });
  const lowerCenterLength0 = $.dimension.curveLength("lowerCenterLength0", { curve: lowerCenterline.segments.byKey.reservoir, value: mm(22) });
  const lowerCenterLength1 = $.dimension.curveLength("lowerCenterLength1", { curve: lowerCenterline.segments.byKey.approach, value: mm(14) });
  const lowerCenterLength2 = $.dimension.curveLength("lowerCenterLength2", { curve: lowerCenterline.segments.byKey.drop, value: mm(126) });

  const lowerSealInsetX = $.geometry.segment("lowerSealInsetX", {
    start: lowerCenterline.vertices.byKey.reservoir,
    end: [-52, -30],
    role: "construction",
  });
  const lowerSealInsetXAxis = $.constraint.horizontal("lowerSealInsetXAxis", { span: lowerSealInsetX.span });
  const lowerSealInsetXLength = $.dimension.curveLength("lowerSealInsetXLength", { curve: lowerSealInsetX.span, value: mm(4) });
  const lowerSealInsetY = $.geometry.segment("lowerSealInsetY", {
    start: lowerSealInsetX.end,
    end: [-52, -24],
    role: "construction",
  });
  const lowerSealInsetYAxis = $.constraint.vertical("lowerSealInsetYAxis", { span: lowerSealInsetY.span });
  const lowerSealInsetYLength = $.dimension.curveLength("lowerSealInsetYLength", { curve: lowerSealInsetY.span, value: mm(6) });
  const lowerSeal = $.geometry.polyline("lowerSeal", {
    vertices: [
      { key: "innerStart", position: [-52, -24] },
      { key: "innerRun", position: [106, -24] },
      { key: "outerPort", position: [106, -50] },
      { key: "outerStart", position: [-52, -50] },
    ],
    closed: true,
  });
  const lowerSealLocated = $.constraint.coincident("lowerSealLocated", {
    first: lowerSeal.vertices.byKey.innerStart,
    second: lowerSealInsetY.end,
  });
  const lowerSealAxis0 = $.constraint.horizontal("lowerSealAxis0", { span: lowerSeal.segments.byKey.innerStart });
  const lowerSealAxis1 = $.constraint.vertical("lowerSealAxis1", { span: lowerSeal.segments.byKey.innerRun });
  const lowerSealAxis2 = $.constraint.horizontal("lowerSealAxis2", { span: lowerSeal.segments.byKey.outerPort });
  const lowerSealAxis3 = $.constraint.vertical("lowerSealAxis3", { span: lowerSeal.segments.byKey.outerStart });
  const lowerSealLength0 = $.dimension.curveLength("lowerSealLength0", { curve: lowerSeal.segments.byKey.innerStart, value: mm(158) });
  const lowerSealLength1 = $.dimension.curveLength("lowerSealLength1", { curve: lowerSeal.segments.byKey.innerRun, value: mm(26) });

  // One AI-authored structural rule rounds every current water-channel corner.
  // O-ring groove centerlines stay simple closed loops for cheap deterministic replay.
  const channelBendRadius = mm(5);
  const upperChannelBends = $.use("upperChannelBends", waterChannel, { corners: upperCenterline.filletableCorners, bendRadius: channelBendRadius });
  const middleChannelBends = $.use("middleChannelBends", waterChannel, { corners: middleCenterline.filletableCorners, bendRadius: channelBendRadius });
  const lowerChannelBends = $.use("lowerChannelBends", waterChannel, { corners: lowerCenterline.filletableCorners, bendRadius: channelBendRadius });

  // Dimensioned construction rails carry eight referenced 5 mm screw centers.
  const topScrewInset = $.geometry.segment("topScrewInset", {
    start: plate.corners[3],
    end: [-120, 53],
    role: "construction",
  });
  const topScrewInsetAxis = $.constraint.vertical("topScrewInsetAxis", { span: topScrewInset.span });
  const topScrewInsetLength = $.dimension.curveLength("topScrewInsetLength", { curve: topScrewInset.span, value: mm(7) });
  const topScrewRail0 = $.geometry.segment("topScrewRail0", { start: topScrewInset.end, end: [-108, 53], role: "construction" });
  const topScrewRail0Axis = $.constraint.horizontal("topScrewRail0Axis", { span: topScrewRail0.span });
  const topScrewRail0Length = $.dimension.curveLength("topScrewRail0Length", { curve: topScrewRail0.span, value: mm(12) });
  const topScrewRail1 = $.geometry.segment("topScrewRail1", { start: topScrewRail0.end, end: [-80, 53], role: "construction" });
  const topScrewRail1Axis = $.constraint.horizontal("topScrewRail1Axis", { span: topScrewRail1.span });
  const topScrewRail1Length = $.dimension.curveLength("topScrewRail1Length", { curve: topScrewRail1.span, value: mm(28) });
  const topScrewRail2 = $.geometry.segment("topScrewRail2", { start: topScrewRail1.end, end: [96, 53], role: "construction" });
  const topScrewRail2Axis = $.constraint.horizontal("topScrewRail2Axis", { span: topScrewRail2.span });
  const topScrewRail2Length = $.dimension.curveLength("topScrewRail2Length", { curve: topScrewRail2.span, value: mm(176) });
  const topScrewRail3 = $.geometry.segment("topScrewRail3", { start: topScrewRail2.end, end: [112, 53], role: "construction" });
  const topScrewRail3Axis = $.constraint.horizontal("topScrewRail3Axis", { span: topScrewRail3.span });
  const topScrewRail3Length = $.dimension.curveLength("topScrewRail3Length", { curve: topScrewRail3.span, value: mm(16) });

  const bottomScrewInset = $.geometry.segment("bottomScrewInset", {
    start: plate.corners[0],
    end: [-120, -53],
    role: "construction",
  });
  const bottomScrewInsetAxis = $.constraint.vertical("bottomScrewInsetAxis", { span: bottomScrewInset.span });
  const bottomScrewInsetLength = $.dimension.curveLength("bottomScrewInsetLength", { curve: bottomScrewInset.span, value: mm(7) });
  const bottomScrewRail0 = $.geometry.segment("bottomScrewRail0", { start: bottomScrewInset.end, end: [-108, -53], role: "construction" });
  const bottomScrewRail0Axis = $.constraint.horizontal("bottomScrewRail0Axis", { span: bottomScrewRail0.span });
  const bottomScrewRail0Length = $.dimension.curveLength("bottomScrewRail0Length", { curve: bottomScrewRail0.span, value: mm(12) });
  const bottomScrewRail1 = $.geometry.segment("bottomScrewRail1", { start: bottomScrewRail0.end, end: [-80, -53], role: "construction" });
  const bottomScrewRail1Axis = $.constraint.horizontal("bottomScrewRail1Axis", { span: bottomScrewRail1.span });
  const bottomScrewRail1Length = $.dimension.curveLength("bottomScrewRail1Length", { curve: bottomScrewRail1.span, value: mm(28) });
  const bottomScrewRail2 = $.geometry.segment("bottomScrewRail2", { start: bottomScrewRail1.end, end: [96, -53], role: "construction" });
  const bottomScrewRail2Axis = $.constraint.horizontal("bottomScrewRail2Axis", { span: bottomScrewRail2.span });
  const bottomScrewRail2Length = $.dimension.curveLength("bottomScrewRail2Length", { curve: bottomScrewRail2.span, value: mm(176) });
  const bottomScrewRail3 = $.geometry.segment("bottomScrewRail3", { start: bottomScrewRail2.end, end: [112, -53], role: "construction" });
  const bottomScrewRail3Axis = $.constraint.horizontal("bottomScrewRail3Axis", { span: bottomScrewRail3.span });
  const bottomScrewRail3Length = $.dimension.curveLength("bottomScrewRail3Length", { curve: bottomScrewRail3.span, value: mm(16) });

  const screwNwOuter = $.geometry.centerRadiusCircle("screwNwOuter", { center: topScrewRail0.end, radius: mm(2.5) });
  const screwNwInner = $.geometry.centerRadiusCircle("screwNwInner", { center: topScrewRail1.end, radius: mm(2.5) });
  const screwNeInner = $.geometry.centerRadiusCircle("screwNeInner", { center: topScrewRail2.end, radius: mm(2.5) });
  const screwNeOuter = $.geometry.centerRadiusCircle("screwNeOuter", { center: topScrewRail3.end, radius: mm(2.5) });
  const screwSwOuter = $.geometry.centerRadiusCircle("screwSwOuter", { center: bottomScrewRail0.end, radius: mm(2.5) });
  const screwSwInner = $.geometry.centerRadiusCircle("screwSwInner", { center: bottomScrewRail1.end, radius: mm(2.5) });
  const screwSeInner = $.geometry.centerRadiusCircle("screwSeInner", { center: bottomScrewRail2.end, radius: mm(2.5) });
  const screwSeOuter = $.geometry.centerRadiusCircle("screwSeOuter", { center: bottomScrewRail3.end, radius: mm(2.5) });
  const screwNwOuterDiameter = $.dimension.diameter("screwNwOuterDiameter", { curve: screwNwOuter.curve, value: mm(5) });
  const screwNwInnerDiameter = $.dimension.diameter("screwNwInnerDiameter", { curve: screwNwInner.curve, value: mm(5) });
  const screwNeInnerDiameter = $.dimension.diameter("screwNeInnerDiameter", { curve: screwNeInner.curve, value: mm(5) });
  const screwNeOuterDiameter = $.dimension.diameter("screwNeOuterDiameter", { curve: screwNeOuter.curve, value: mm(5) });
  const screwSwOuterDiameter = $.dimension.diameter("screwSwOuterDiameter", { curve: screwSwOuter.curve, value: mm(5) });
  const screwSwInnerDiameter = $.dimension.diameter("screwSwInnerDiameter", { curve: screwSwInner.curve, value: mm(5) });
  const screwSeInnerDiameter = $.dimension.diameter("screwSeInnerDiameter", { curve: screwSeInner.curve, value: mm(5) });
  const screwSeOuterDiameter = $.dimension.diameter("screwSeOuterDiameter", { curve: screwSeOuter.curve, value: mm(5) });

  $.group("Construction datums", [
    reservoirInsetX,
    reservoirInsetY,
    lowerOutletDatum,
    middleOutletDatum,
    upperOutletDatum,
    upperSealInsetX,
    upperSealInsetY,
    middleSealInsetX,
    middleSealInsetY,
    lowerSealInsetX,
    lowerSealInsetY,
    topScrewInset,
    topScrewRail0,
    topScrewRail1,
    topScrewRail2,
    topScrewRail3,
    bottomScrewInset,
    bottomScrewRail0,
    bottomScrewRail1,
    bottomScrewRail2,
    bottomScrewRail3,
  ]);
  $.group("Acrylic manifold", [
    plate,
    reservoir,
    upperCenterline,
    upperSeal,
    middleCenterline,
    middleSeal,
    lowerCenterline,
    lowerSeal,
    screwNwOuter,
    screwNwInner,
    screwNeInner,
    screwNeOuter,
    screwSwOuter,
    screwSwInner,
    screwSeInner,
    screwSeOuter,
  ]);
  return {
    plate,
    reservoir,
    upperCenterline,
    upperSeal,
    upperChannelBends,
    middleCenterline,
    middleSeal,
    middleChannelBends,
    lowerCenterline,
    lowerSeal,
    lowerChannelBends,
    screwNwOuter,
    screwNwInner,
    screwNeInner,
    screwNeOuter,
    screwSwOuter,
    screwSwInner,
    screwSeInner,
    screwSeOuter,
    plateAnchor,
  };
});
