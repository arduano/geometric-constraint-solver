// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";
import { bridgeCables } from "./patches/bridge-cables.patch.ts";

export default sketch(($) => {
  const deckLeft = $.geometry.line("deckLeft", { start: [-65, 0], end: [-28, 0] });
  const deckCenter = $.geometry.line("deckCenter", { start: deckLeft.end, end: [28, 0] });
  const deckRight = $.geometry.line("deckRight", { start: deckCenter.end, end: [65, 0] });
  const leftTower = $.geometry.line("leftTower", { start: deckLeft.end, end: [-28, 38] });
  const rightTower = $.geometry.line("rightTower", { start: deckCenter.end, end: [28, 38] });
  const cables = $.use("cables", bridgeCables, {
    leftAbutment: deckLeft.start,
    leftBase: leftTower.start,
    leftPeak: leftTower.end,
    rightBase: rightTower.start,
    rightPeak: rightTower.end,
    rightAbutment: deckRight.end,
  });
  const deckLeftAxis = $.constraint.horizontal("deckLeftAxis", { curve: deckLeft });
  const deckCenterAxis = $.constraint.horizontal("deckCenterAxis", { curve: deckCenter });
  const deckRightAxis = $.constraint.horizontal("deckRightAxis", { curve: deckRight });
  const leftTowerAxis = $.constraint.vertical("leftTowerAxis", { curve: leftTower });
  const rightTowerAxis = $.constraint.vertical("rightTowerAxis", { curve: rightTower });
  $.organize("Suspension bridge", [deckLeft, deckCenter, deckRight, leftTower, rightTower, cables, deckLeftAxis, deckCenterAxis, deckRightAxis, leftTowerAxis, rightTowerAxis]);
  return $.outputs({ deckLeft, deckCenter, deckRight, leftTower, rightTower, cables, crown: cables.mainCable.crown, fallingStay: cables.stays.falling });
});
