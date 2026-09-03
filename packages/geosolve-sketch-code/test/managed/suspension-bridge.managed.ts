// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";
import { bridgeCables } from "./patches/bridge-cables.patch.ts";

export default sketch(($) => {
  const deckLeft = $.geometry.segment("deckLeft", { start: [-65, 0], end: [-28, 0] });
  const deckCenter = $.geometry.segment("deckCenter", { start: deckLeft.end, end: [28, 0] });
  const deckRight = $.geometry.segment("deckRight", { start: deckCenter.end, end: [65, 0] });
  const leftTower = $.geometry.segment("leftTower", { start: deckLeft.end, end: [-28, 38] });
  const rightTower = $.geometry.segment("rightTower", { start: deckCenter.end, end: [28, 38] });
  const cables = $.use("cables", bridgeCables, {
    leftAbutment: deckLeft.start,
    leftBase: leftTower.start,
    leftPeak: leftTower.end,
    rightBase: rightTower.start,
    rightPeak: rightTower.end,
    rightAbutment: deckRight.end,
  });
  const deckLeftAxis = $.constraint.horizontal("deckLeftAxis", { span: deckLeft.span });
  const deckCenterAxis = $.constraint.horizontal("deckCenterAxis", { span: deckCenter.span });
  const deckRightAxis = $.constraint.horizontal("deckRightAxis", { span: deckRight.span });
  const leftTowerAxis = $.constraint.vertical("leftTowerAxis", { span: leftTower.span });
  const rightTowerAxis = $.constraint.vertical("rightTowerAxis", { span: rightTower.span });
  $.group("Suspension bridge", [
    deckLeft,
    deckCenter,
    deckRight,
    leftTower,
    rightTower,
    deckLeftAxis,
    deckCenterAxis,
    deckRightAxis,
    leftTowerAxis,
    rightTowerAxis,
  ]);
  return { deckLeft, deckCenter, deckRight, leftTower, rightTower, cables, crown: cables.mainCable.crown, fallingStay: cables.stays.falling };
});
