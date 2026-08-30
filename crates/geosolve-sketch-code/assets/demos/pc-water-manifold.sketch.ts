// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { waterChannel } from "./patches/water-channel.patch.ts";

export default sketch(($) => {
  // One absolute anchor plus driving dimensions locates the complete 240 × 120 mm plate.
  const plate = $.geometry.rectangle("plate", {
    lowerLeft: [-120, -60],
    upperRight: [120, 60],
  });
  const plateAnchor = $.constraint.fixedPoint("plateAnchor", {
    point: plate.corners.lowerLeft,
    target: [-120, -60],
  });
  const plateWidth = $.dimension.curveLength("plateWidth", {
    curve: plate.edges.bottom,
    target: mm(240),
  });
  const plateHeight = $.dimension.curveLength("plateHeight", {
    curve: plate.edges.right,
    target: mm(120),
  });

  // Construction datums place a 60 × 84 mm reservoir bay without coordinate locks.
  const reservoirInsetX = $.geometry.line("reservoirInsetX", {
    start: plate.corners.lowerLeft,
    end: [-108, -60],
    role: "construction",
  });
  const reservoirInsetXAxis = $.constraint.horizontal("reservoirInsetXAxis", {
    curve: reservoirInsetX.span,
  });
  const reservoirInsetXLength = $.dimension.curveLength("reservoirInsetXLength", {
    curve: reservoirInsetX.span,
    target: mm(12),
  });
  const reservoirInsetY = $.geometry.line("reservoirInsetY", {
    start: reservoirInsetX.end,
    end: [-108, -42],
    role: "construction",
  });
  const reservoirInsetYAxis = $.constraint.vertical("reservoirInsetYAxis", {
    curve: reservoirInsetY.span,
  });
  const reservoirInsetYLength = $.dimension.curveLength("reservoirInsetYLength", {
    curve: reservoirInsetY.span,
    target: mm(18),
  });
  const reservoir = $.geometry.rectangle("reservoir", {
    lowerLeft: [-108, -42],
    upperRight: [-48, 42],
  });
  const reservoirLocated = $.constraint.coincident("reservoirLocated", {
    first: reservoir.corners.lowerLeft,
    second: reservoirInsetY.end,
  });
  const reservoirWidth = $.dimension.curveLength("reservoirWidth", {
    curve: reservoir.edges.bottom,
    target: mm(60),
  });
  const reservoirHeight = $.dimension.curveLength("reservoirHeight", {
    curve: reservoir.edges.right,
    target: mm(84),
  });

  // A dimensioned construction spine locates all three reservoir outlets.
  const lowerOutletDatum = $.geometry.line("lowerOutletDatum", {
    start: reservoir.corners.lowerRight,
    end: [-48, -30],
    role: "construction",
  });
  const lowerOutletAxis = $.constraint.vertical("lowerOutletAxis", {
    curve: lowerOutletDatum.span,
  });
  const lowerOutletLength = $.dimension.curveLength("lowerOutletLength", {
    curve: lowerOutletDatum.span,
    target: mm(12),
  });
  const middleOutletDatum = $.geometry.line("middleOutletDatum", {
    start: lowerOutletDatum.end,
    end: [-48, 0],
    role: "construction",
  });
  const middleOutletAxis = $.constraint.vertical("middleOutletAxis", {
    curve: middleOutletDatum.span,
  });
  const middleOutletLength = $.dimension.curveLength("middleOutletLength", {
    curve: middleOutletDatum.span,
    target: mm(30),
  });
  const upperOutletDatum = $.geometry.line("upperOutletDatum", {
    start: middleOutletDatum.end,
    end: [-48, 30],
    role: "construction",
  });
  const upperOutletAxis = $.constraint.vertical("upperOutletAxis", {
    curve: upperOutletDatum.span,
  });
  const upperOutletLength = $.dimension.curveLength("upperOutletLength", {
    curve: upperOutletDatum.span,
    target: mm(30),
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
  const upperCenterAxis0 = $.constraint.horizontal("upperCenterAxis0", { curve: upperCenterline.segments.byKey.reservoir });
  const upperCenterAxis1 = $.constraint.vertical("upperCenterAxis1", { curve: upperCenterline.segments.byKey.approach });
  const upperCenterAxis2 = $.constraint.horizontal("upperCenterAxis2", { curve: upperCenterline.segments.byKey.rise });
  const upperCenterLength0 = $.dimension.curveLength("upperCenterLength0", { curve: upperCenterline.segments.byKey.reservoir, target: mm(22) });
  const upperCenterLength1 = $.dimension.curveLength("upperCenterLength1", { curve: upperCenterline.segments.byKey.approach, target: mm(14) });
  const upperCenterLength2 = $.dimension.curveLength("upperCenterLength2", { curve: upperCenterline.segments.byKey.rise, target: mm(126) });

  const upperSealInsetX = $.geometry.line("upperSealInsetX", {
    start: upperCenterline.vertices.byKey.reservoir,
    end: [-52, 30],
    role: "construction",
  });
  const upperSealInsetXAxis = $.constraint.horizontal("upperSealInsetXAxis", { curve: upperSealInsetX.span });
  const upperSealInsetXLength = $.dimension.curveLength("upperSealInsetXLength", { curve: upperSealInsetX.span, target: mm(4) });
  const upperSealInsetY = $.geometry.line("upperSealInsetY", {
    start: upperSealInsetX.end,
    end: [-52, 24],
    role: "construction",
  });
  const upperSealInsetYAxis = $.constraint.vertical("upperSealInsetYAxis", { curve: upperSealInsetY.span });
  const upperSealInsetYLength = $.dimension.curveLength("upperSealInsetYLength", { curve: upperSealInsetY.span, target: mm(6) });
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
  const upperSealAxis0 = $.constraint.horizontal("upperSealAxis0", { curve: upperSeal.segments.byKey.innerStart });
  const upperSealAxis1 = $.constraint.vertical("upperSealAxis1", { curve: upperSeal.segments.byKey.innerRun });
  const upperSealAxis2 = $.constraint.horizontal("upperSealAxis2", { curve: upperSeal.segments.byKey.outerPort });
  const upperSealAxis3 = $.constraint.vertical("upperSealAxis3", { curve: upperSeal.segments.byKey.outerStart });
  const upperSealLength0 = $.dimension.curveLength("upperSealLength0", { curve: upperSeal.segments.byKey.innerStart, target: mm(158) });
  const upperSealLength1 = $.dimension.curveLength("upperSealLength1", { curve: upperSeal.segments.byKey.innerRun, target: mm(26) });

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
  const middleCenterAxis0 = $.constraint.horizontal("middleCenterAxis0", { curve: middleCenterline.segments.byKey.reservoir });
  const middleCenterAxis1 = $.constraint.vertical("middleCenterAxis1", { curve: middleCenterline.segments.byKey.approach });
  const middleCenterAxis2 = $.constraint.horizontal("middleCenterAxis2", { curve: middleCenterline.segments.byKey.rise });
  const middleCenterLength0 = $.dimension.curveLength("middleCenterLength0", { curve: middleCenterline.segments.byKey.reservoir, target: mm(44) });
  const middleCenterLength1 = $.dimension.curveLength("middleCenterLength1", { curve: middleCenterline.segments.byKey.approach, target: mm(14) });
  const middleCenterLength2 = $.dimension.curveLength("middleCenterLength2", { curve: middleCenterline.segments.byKey.rise, target: mm(104) });

  const middleSealInsetX = $.geometry.line("middleSealInsetX", {
    start: middleCenterline.vertices.byKey.reservoir,
    end: [-52, 0],
    role: "construction",
  });
  const middleSealInsetXAxis = $.constraint.horizontal("middleSealInsetXAxis", { curve: middleSealInsetX.span });
  const middleSealInsetXLength = $.dimension.curveLength("middleSealInsetXLength", { curve: middleSealInsetX.span, target: mm(4) });
  const middleSealInsetY = $.geometry.line("middleSealInsetY", {
    start: middleSealInsetX.end,
    end: [-52, -6],
    role: "construction",
  });
  const middleSealInsetYAxis = $.constraint.vertical("middleSealInsetYAxis", { curve: middleSealInsetY.span });
  const middleSealInsetYLength = $.dimension.curveLength("middleSealInsetYLength", { curve: middleSealInsetY.span, target: mm(6) });
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
  const middleSealAxis0 = $.constraint.horizontal("middleSealAxis0", { curve: middleSeal.segments.byKey.innerStart });
  const middleSealAxis1 = $.constraint.vertical("middleSealAxis1", { curve: middleSeal.segments.byKey.innerRun });
  const middleSealAxis2 = $.constraint.horizontal("middleSealAxis2", { curve: middleSeal.segments.byKey.outerPort });
  const middleSealAxis3 = $.constraint.vertical("middleSealAxis3", { curve: middleSeal.segments.byKey.outerStart });
  const middleSealLength0 = $.dimension.curveLength("middleSealLength0", { curve: middleSeal.segments.byKey.innerStart, target: mm(158) });
  const middleSealLength1 = $.dimension.curveLength("middleSealLength1", { curve: middleSeal.segments.byKey.innerRun, target: mm(26) });

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
  const lowerCenterAxis0 = $.constraint.horizontal("lowerCenterAxis0", { curve: lowerCenterline.segments.byKey.reservoir });
  const lowerCenterAxis1 = $.constraint.vertical("lowerCenterAxis1", { curve: lowerCenterline.segments.byKey.approach });
  const lowerCenterAxis2 = $.constraint.horizontal("lowerCenterAxis2", { curve: lowerCenterline.segments.byKey.drop });
  const lowerCenterLength0 = $.dimension.curveLength("lowerCenterLength0", { curve: lowerCenterline.segments.byKey.reservoir, target: mm(22) });
  const lowerCenterLength1 = $.dimension.curveLength("lowerCenterLength1", { curve: lowerCenterline.segments.byKey.approach, target: mm(14) });
  const lowerCenterLength2 = $.dimension.curveLength("lowerCenterLength2", { curve: lowerCenterline.segments.byKey.drop, target: mm(126) });

  const lowerSealInsetX = $.geometry.line("lowerSealInsetX", {
    start: lowerCenterline.vertices.byKey.reservoir,
    end: [-52, -30],
    role: "construction",
  });
  const lowerSealInsetXAxis = $.constraint.horizontal("lowerSealInsetXAxis", { curve: lowerSealInsetX.span });
  const lowerSealInsetXLength = $.dimension.curveLength("lowerSealInsetXLength", { curve: lowerSealInsetX.span, target: mm(4) });
  const lowerSealInsetY = $.geometry.line("lowerSealInsetY", {
    start: lowerSealInsetX.end,
    end: [-52, -24],
    role: "construction",
  });
  const lowerSealInsetYAxis = $.constraint.vertical("lowerSealInsetYAxis", { curve: lowerSealInsetY.span });
  const lowerSealInsetYLength = $.dimension.curveLength("lowerSealInsetYLength", { curve: lowerSealInsetY.span, target: mm(6) });
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
  const lowerSealAxis0 = $.constraint.horizontal("lowerSealAxis0", { curve: lowerSeal.segments.byKey.innerStart });
  const lowerSealAxis1 = $.constraint.vertical("lowerSealAxis1", { curve: lowerSeal.segments.byKey.innerRun });
  const lowerSealAxis2 = $.constraint.horizontal("lowerSealAxis2", { curve: lowerSeal.segments.byKey.outerPort });
  const lowerSealAxis3 = $.constraint.vertical("lowerSealAxis3", { curve: lowerSeal.segments.byKey.outerStart });
  const lowerSealLength0 = $.dimension.curveLength("lowerSealLength0", { curve: lowerSeal.segments.byKey.innerStart, target: mm(158) });
  const lowerSealLength1 = $.dimension.curveLength("lowerSealLength1", { curve: lowerSeal.segments.byKey.innerRun, target: mm(26) });

  // One AI-authored structural rule rounds every current water-channel corner.
  // O-ring groove centerlines stay simple closed loops for cheap deterministic replay.
  const channelBendRadius = mm(5);
  const upperChannelBends = $.use("upperChannelBends", waterChannel, { corners: upperCenterline.filletableCorners, bendRadius: channelBendRadius });
  const middleChannelBends = $.use("middleChannelBends", waterChannel, { corners: middleCenterline.filletableCorners, bendRadius: channelBendRadius });
  const lowerChannelBends = $.use("lowerChannelBends", waterChannel, { corners: lowerCenterline.filletableCorners, bendRadius: channelBendRadius });

  // Dimensioned construction rails carry eight referenced 5 mm screw centers.
  const topScrewInset = $.geometry.line("topScrewInset", {
    start: plate.corners.upperLeft,
    end: [-120, 53],
    role: "construction",
  });
  const topScrewInsetAxis = $.constraint.vertical("topScrewInsetAxis", { curve: topScrewInset.span });
  const topScrewInsetLength = $.dimension.curveLength("topScrewInsetLength", { curve: topScrewInset.span, target: mm(7) });
  const topScrewRail0 = $.geometry.line("topScrewRail0", { start: topScrewInset.end, end: [-108, 53], role: "construction" });
  const topScrewRail0Axis = $.constraint.horizontal("topScrewRail0Axis", { curve: topScrewRail0.span });
  const topScrewRail0Length = $.dimension.curveLength("topScrewRail0Length", { curve: topScrewRail0.span, target: mm(12) });
  const topScrewRail1 = $.geometry.line("topScrewRail1", { start: topScrewRail0.end, end: [-80, 53], role: "construction" });
  const topScrewRail1Axis = $.constraint.horizontal("topScrewRail1Axis", { curve: topScrewRail1.span });
  const topScrewRail1Length = $.dimension.curveLength("topScrewRail1Length", { curve: topScrewRail1.span, target: mm(28) });
  const topScrewRail2 = $.geometry.line("topScrewRail2", { start: topScrewRail1.end, end: [96, 53], role: "construction" });
  const topScrewRail2Axis = $.constraint.horizontal("topScrewRail2Axis", { curve: topScrewRail2.span });
  const topScrewRail2Length = $.dimension.curveLength("topScrewRail2Length", { curve: topScrewRail2.span, target: mm(176) });
  const topScrewRail3 = $.geometry.line("topScrewRail3", { start: topScrewRail2.end, end: [112, 53], role: "construction" });
  const topScrewRail3Axis = $.constraint.horizontal("topScrewRail3Axis", { curve: topScrewRail3.span });
  const topScrewRail3Length = $.dimension.curveLength("topScrewRail3Length", { curve: topScrewRail3.span, target: mm(16) });

  const bottomScrewInset = $.geometry.line("bottomScrewInset", {
    start: plate.corners.lowerLeft,
    end: [-120, -53],
    role: "construction",
  });
  const bottomScrewInsetAxis = $.constraint.vertical("bottomScrewInsetAxis", { curve: bottomScrewInset.span });
  const bottomScrewInsetLength = $.dimension.curveLength("bottomScrewInsetLength", { curve: bottomScrewInset.span, target: mm(7) });
  const bottomScrewRail0 = $.geometry.line("bottomScrewRail0", { start: bottomScrewInset.end, end: [-108, -53], role: "construction" });
  const bottomScrewRail0Axis = $.constraint.horizontal("bottomScrewRail0Axis", { curve: bottomScrewRail0.span });
  const bottomScrewRail0Length = $.dimension.curveLength("bottomScrewRail0Length", { curve: bottomScrewRail0.span, target: mm(12) });
  const bottomScrewRail1 = $.geometry.line("bottomScrewRail1", { start: bottomScrewRail0.end, end: [-80, -53], role: "construction" });
  const bottomScrewRail1Axis = $.constraint.horizontal("bottomScrewRail1Axis", { curve: bottomScrewRail1.span });
  const bottomScrewRail1Length = $.dimension.curveLength("bottomScrewRail1Length", { curve: bottomScrewRail1.span, target: mm(28) });
  const bottomScrewRail2 = $.geometry.line("bottomScrewRail2", { start: bottomScrewRail1.end, end: [96, -53], role: "construction" });
  const bottomScrewRail2Axis = $.constraint.horizontal("bottomScrewRail2Axis", { curve: bottomScrewRail2.span });
  const bottomScrewRail2Length = $.dimension.curveLength("bottomScrewRail2Length", { curve: bottomScrewRail2.span, target: mm(176) });
  const bottomScrewRail3 = $.geometry.line("bottomScrewRail3", { start: bottomScrewRail2.end, end: [112, -53], role: "construction" });
  const bottomScrewRail3Axis = $.constraint.horizontal("bottomScrewRail3Axis", { curve: bottomScrewRail3.span });
  const bottomScrewRail3Length = $.dimension.curveLength("bottomScrewRail3Length", { curve: bottomScrewRail3.span, target: mm(16) });

  const screwNwOuter = $.geometry.circle("screwNwOuter", { center: topScrewRail0.end, radius: mm(2.5) });
  const screwNwInner = $.geometry.circle("screwNwInner", { center: topScrewRail1.end, radius: mm(2.5) });
  const screwNeInner = $.geometry.circle("screwNeInner", { center: topScrewRail2.end, radius: mm(2.5) });
  const screwNeOuter = $.geometry.circle("screwNeOuter", { center: topScrewRail3.end, radius: mm(2.5) });
  const screwSwOuter = $.geometry.circle("screwSwOuter", { center: bottomScrewRail0.end, radius: mm(2.5) });
  const screwSwInner = $.geometry.circle("screwSwInner", { center: bottomScrewRail1.end, radius: mm(2.5) });
  const screwSeInner = $.geometry.circle("screwSeInner", { center: bottomScrewRail2.end, radius: mm(2.5) });
  const screwSeOuter = $.geometry.circle("screwSeOuter", { center: bottomScrewRail3.end, radius: mm(2.5) });
  const screwNwOuterDiameter = $.dimension.diameter("screwNwOuterDiameter", { curve: screwNwOuter.circle, target: mm(5) });
  const screwNwInnerDiameter = $.dimension.diameter("screwNwInnerDiameter", { curve: screwNwInner.circle, target: mm(5) });
  const screwNeInnerDiameter = $.dimension.diameter("screwNeInnerDiameter", { curve: screwNeInner.circle, target: mm(5) });
  const screwNeOuterDiameter = $.dimension.diameter("screwNeOuterDiameter", { curve: screwNeOuter.circle, target: mm(5) });
  const screwSwOuterDiameter = $.dimension.diameter("screwSwOuterDiameter", { curve: screwSwOuter.circle, target: mm(5) });
  const screwSwInnerDiameter = $.dimension.diameter("screwSwInnerDiameter", { curve: screwSwInner.circle, target: mm(5) });
  const screwSeInnerDiameter = $.dimension.diameter("screwSeInnerDiameter", { curve: screwSeInner.circle, target: mm(5) });
  const screwSeOuterDiameter = $.dimension.diameter("screwSeOuterDiameter", { curve: screwSeOuter.circle, target: mm(5) });

  $.organize("Construction datums", [
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
  $.organize("Acrylic manifold", [
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
  ]);
  return $.outputs({
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
  });
});
