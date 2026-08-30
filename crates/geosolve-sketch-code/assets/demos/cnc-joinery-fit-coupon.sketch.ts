// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { cornerReliefs } from "./patches/corner-reliefs.patch.ts";
import { fillets } from "./patches/fillet-record.patch.ts";

export default sketch(($) => {
  // A sacrificial router coupon compares one nominal 18 mm tab against loose,
  // nominal and press-fit mortises before committing joinery to finished stock.
  // The circles are conservative corner-centred overcuts, not inferred CAM or
  // automatic cutter compensation.
  const femaleBlank = $.geometry.rectangle("femaleBlank", {
    lowerLeft: [-130, -70],
    upperRight: [-10, 70],
  });
  const femaleBlankAnchor = $.constraint.fixedPoint("femaleBlankAnchor", {
    point: femaleBlank.corners.lowerLeft,
    target: [-130, -70],
  });
  const femaleBlankWidth = $.dimension.curveLength("femaleBlankWidth", {
    curve: femaleBlank.edges.bottom,
    target: mm(120),
  });
  const femaleBlankHeight = $.dimension.curveLength("femaleBlankHeight", {
    curve: femaleBlank.edges.left,
    target: mm(140),
  });

  const looseMortise = $.geometry.polyline("looseMortise", {
    vertices: [
      { key: "lowerLeft", position: [-105, 32.8] },
      { key: "lowerRight", position: [-35, 32.8] },
      { key: "upperRight", position: [-35, 51.2] },
      { key: "upperLeft", position: [-105, 51.2] },
    ],
    closed: true,
  });
  const looseMortiseBottom = $.constraint.horizontal("looseMortiseBottom", { curve: looseMortise.segments.byKey.lowerLeft });
  const looseMortiseRight = $.constraint.vertical("looseMortiseRight", { curve: looseMortise.segments.byKey.lowerRight });
  const looseMortiseTop = $.constraint.horizontal("looseMortiseTop", { curve: looseMortise.segments.byKey.upperRight });
  const looseMortiseLeft = $.constraint.vertical("looseMortiseLeft", { curve: looseMortise.segments.byKey.upperLeft });
  const looseMortiseWidth = $.dimension.curveLength("looseMortiseWidth", { curve: looseMortise.segments.byKey.lowerLeft, target: mm(70) });
  const looseMortiseHeight = $.dimension.curveLength("looseMortiseHeight", { curve: looseMortise.segments.byKey.lowerRight, target: mm(18.4) });

  const nominalMortise = $.geometry.polyline("nominalMortise", {
    vertices: [
      { key: "lowerLeft", position: [-105, -9] },
      { key: "lowerRight", position: [-35, -9] },
      { key: "upperRight", position: [-35, 9] },
      { key: "upperLeft", position: [-105, 9] },
    ],
    closed: true,
  });
  const nominalMortiseBottom = $.constraint.horizontal("nominalMortiseBottom", { curve: nominalMortise.segments.byKey.lowerLeft });
  const nominalMortiseRight = $.constraint.vertical("nominalMortiseRight", { curve: nominalMortise.segments.byKey.lowerRight });
  const nominalMortiseTop = $.constraint.horizontal("nominalMortiseTop", { curve: nominalMortise.segments.byKey.upperRight });
  const nominalMortiseLeft = $.constraint.vertical("nominalMortiseLeft", { curve: nominalMortise.segments.byKey.upperLeft });
  const nominalMortiseWidth = $.dimension.curveLength("nominalMortiseWidth", { curve: nominalMortise.segments.byKey.lowerLeft, target: mm(70) });
  const nominalMortiseHeight = $.dimension.curveLength("nominalMortiseHeight", { curve: nominalMortise.segments.byKey.lowerRight, target: mm(18) });

  const pressMortise = $.geometry.polyline("pressMortise", {
    vertices: [
      { key: "lowerLeft", position: [-105, -50.8] },
      { key: "lowerRight", position: [-35, -50.8] },
      { key: "upperRight", position: [-35, -33.2] },
      { key: "upperLeft", position: [-105, -33.2] },
    ],
    closed: true,
  });
  const pressMortiseBottom = $.constraint.horizontal("pressMortiseBottom", { curve: pressMortise.segments.byKey.lowerLeft });
  const pressMortiseRight = $.constraint.vertical("pressMortiseRight", { curve: pressMortise.segments.byKey.lowerRight });
  const pressMortiseTop = $.constraint.horizontal("pressMortiseTop", { curve: pressMortise.segments.byKey.upperRight });
  const pressMortiseLeft = $.constraint.vertical("pressMortiseLeft", { curve: pressMortise.segments.byKey.upperLeft });
  const pressMortiseWidth = $.dimension.curveLength("pressMortiseWidth", { curve: pressMortise.segments.byKey.lowerLeft, target: mm(70) });
  const pressMortiseHeight = $.dimension.curveLength("pressMortiseHeight", { curve: pressMortise.segments.byKey.lowerRight, target: mm(17.6) });

  const cutterRadius = mm(3.175);
  const looseReliefs = $.use("looseReliefs", cornerReliefs, {
    centers: {
      lowerLeft: looseMortise.vertices.byKey.lowerLeft,
      lowerRight: looseMortise.vertices.byKey.lowerRight,
      upperRight: looseMortise.vertices.byKey.upperRight,
      upperLeft: looseMortise.vertices.byKey.upperLeft,
    },
    radius: cutterRadius,
  });
  const nominalReliefs = $.use("nominalReliefs", cornerReliefs, {
    centers: {
      lowerLeft: nominalMortise.vertices.byKey.lowerLeft,
      lowerRight: nominalMortise.vertices.byKey.lowerRight,
      upperRight: nominalMortise.vertices.byKey.upperRight,
      upperLeft: nominalMortise.vertices.byKey.upperLeft,
    },
    radius: cutterRadius,
  });
  const pressReliefs = $.use("pressReliefs", cornerReliefs, {
    centers: {
      lowerLeft: pressMortise.vertices.byKey.lowerLeft,
      lowerRight: pressMortise.vertices.byKey.lowerRight,
      upperRight: pressMortise.vertices.byKey.upperRight,
      upperLeft: pressMortise.vertices.byKey.upperLeft,
    },
    radius: cutterRadius,
  });

  const looseTab = $.geometry.rectangle("looseTab", { lowerLeft: [20, 33], upperRight: [115, 51] });
  const nominalTab = $.geometry.rectangle("nominalTab", { lowerLeft: [20, -9], upperRight: [115, 9] });
  const pressTab = $.geometry.rectangle("pressTab", { lowerLeft: [20, -51], upperRight: [115, -33] });
  const looseTabLength = $.dimension.curveLength("looseTabLength", { curve: looseTab.edges.bottom, target: mm(95) });
  const nominalTabLength = $.dimension.curveLength("nominalTabLength", { curve: nominalTab.edges.bottom, target: mm(95) });
  const pressTabLength = $.dimension.curveLength("pressTabLength", { curve: pressTab.edges.bottom, target: mm(95) });
  const looseTabThickness = $.dimension.curveLength("looseTabThickness", { curve: looseTab.edges.right, target: mm(18) });
  const nominalTabThickness = $.dimension.curveLength("nominalTabThickness", { curve: nominalTab.edges.right, target: mm(18) });
  const pressTabThickness = $.dimension.curveLength("pressTabThickness", { curve: pressTab.edges.right, target: mm(18) });

  // One absolute point plus relational construction datums place every fit
  // station. Literal positions above are only solver seeds, never authority.
  const blankToStationX = $.geometry.line("blankToStationX", {
    start: femaleBlank.corners.lowerLeft,
    end: [-105, -70],
    role: "construction",
  });
  const blankToStationXHorizontal = $.constraint.horizontal("blankToStationXHorizontal", { curve: blankToStationX.span });
  const blankToStationXLength = $.dimension.curveLength("blankToStationXLength", { curve: blankToStationX.span, target: mm(25) });
  const pressMortiseY = $.geometry.line("pressMortiseY", {
    start: blankToStationX.end,
    end: pressMortise.vertices.byKey.lowerLeft,
    role: "construction",
  });
  const pressMortiseYVertical = $.constraint.vertical("pressMortiseYVertical", { curve: pressMortiseY.span });
  const pressMortiseYLength = $.dimension.curveLength("pressMortiseYLength", { curve: pressMortiseY.span, target: mm(19.2) });
  const pressToNominalMortise = $.geometry.line("pressToNominalMortise", {
    start: pressMortise.vertices.byKey.lowerLeft,
    end: nominalMortise.vertices.byKey.lowerLeft,
    role: "construction",
  });
  const pressToNominalMortiseVertical = $.constraint.vertical("pressToNominalMortiseVertical", { curve: pressToNominalMortise.span });
  const pressToNominalMortiseLength = $.dimension.curveLength("pressToNominalMortiseLength", { curve: pressToNominalMortise.span, target: mm(41.8) });
  const nominalToLooseMortise = $.geometry.line("nominalToLooseMortise", {
    start: nominalMortise.vertices.byKey.lowerLeft,
    end: looseMortise.vertices.byKey.lowerLeft,
    role: "construction",
  });
  const nominalToLooseMortiseVertical = $.constraint.vertical("nominalToLooseMortiseVertical", { curve: nominalToLooseMortise.span });
  const nominalToLooseMortiseLength = $.dimension.curveLength("nominalToLooseMortiseLength", { curve: nominalToLooseMortise.span, target: mm(41.8) });
  const nominalStationWidth = $.geometry.line("nominalStationWidth", {
    start: nominalMortise.vertices.byKey.lowerLeft,
    end: nominalTab.corners.lowerLeft,
    role: "construction",
  });
  const nominalStationWidthHorizontal = $.constraint.horizontal("nominalStationWidthHorizontal", { curve: nominalStationWidth.span });
  const nominalStationWidthLength = $.dimension.curveLength("nominalStationWidthLength", { curve: nominalStationWidth.span, target: mm(125) });
  const pressToNominalTab = $.geometry.line("pressToNominalTab", {
    start: pressTab.corners.lowerLeft,
    end: nominalTab.corners.lowerLeft,
    role: "construction",
  });
  const pressToNominalTabVertical = $.constraint.vertical("pressToNominalTabVertical", { curve: pressToNominalTab.span });
  const pressToNominalTabLength = $.dimension.curveLength("pressToNominalTabLength", { curve: pressToNominalTab.span, target: mm(42) });
  const nominalToLooseTab = $.geometry.line("nominalToLooseTab", {
    start: nominalTab.corners.lowerLeft,
    end: looseTab.corners.lowerLeft,
    role: "construction",
  });
  const nominalToLooseTabVertical = $.constraint.vertical("nominalToLooseTabVertical", { curve: nominalToLooseTab.span });
  const nominalToLooseTabLength = $.dimension.curveLength("nominalToLooseTabLength", { curve: nominalToLooseTab.span, target: mm(42) });

  const edgeRadius = mm(6);
  const blankHandling = $.use("blankHandling", fillets, {
    corners: {
      lowerLeft: femaleBlank.corners.lowerLeft,
      lowerRight: femaleBlank.corners.lowerRight,
      upperRight: femaleBlank.corners.upperRight,
      upperLeft: femaleBlank.corners.upperLeft,
    },
    radius: edgeRadius,
  });
  const looseTabHandling = $.use("looseTabHandling", fillets, {
    corners: { lowerRight: looseTab.corners.lowerRight, upperRight: looseTab.corners.upperRight },
    radius: edgeRadius,
  });
  const nominalTabHandling = $.use("nominalTabHandling", fillets, {
    corners: { lowerRight: nominalTab.corners.lowerRight, upperRight: nominalTab.corners.upperRight },
    radius: edgeRadius,
  });
  const pressTabHandling = $.use("pressTabHandling", fillets, {
    corners: { lowerRight: pressTab.corners.lowerRight, upperRight: pressTab.corners.upperRight },
    radius: edgeRadius,
  });

  $.organize("Female coupon blank", [femaleBlank, femaleBlankAnchor, femaleBlankWidth, femaleBlankHeight, blankHandling]);
  $.organize("Relational placement", [blankToStationX, pressMortiseY, pressToNominalMortise, nominalToLooseMortise, nominalStationWidth, pressToNominalTab, nominalToLooseTab]);
  $.organize("Loose fit", [looseMortise, looseReliefs, looseTab, looseTabHandling]);
  $.organize("Nominal fit", [nominalMortise, nominalReliefs, nominalTab, nominalTabHandling]);
  $.organize("Press fit", [pressMortise, pressReliefs, pressTab, pressTabHandling]);

  return $.outputs({
    femaleBlank,
    looseMortise,
    nominalMortise,
    pressMortise,
    looseReliefs: looseReliefs.reliefs,
    nominalReliefs: nominalReliefs.reliefs,
    pressReliefs: pressReliefs.reliefs,
    looseTab,
    nominalTab,
    pressTab,
    blankHandling: blankHandling.fillets,
    looseTabHandling: looseTabHandling.fillets,
    nominalTabHandling: nominalTabHandling.fillets,
    pressTabHandling: pressTabHandling.fillets,
  });
});
