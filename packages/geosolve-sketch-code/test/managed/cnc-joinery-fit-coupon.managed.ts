// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
import { cornerReliefs } from "./patches/corner-reliefs.patch.ts";
import { fillets } from "./patches/fillet-record.patch.ts";

export default sketch(($) => {
  // A sacrificial router coupon compares one nominal 18 mm tab against loose,
  // nominal and press-fit mortises before committing joinery to finished stock.
  // The circles are conservative corner-centred overcuts, not inferred CAM or
  // automatic cutter compensation.
  const femaleBlank = $.geometry.polyline("femaleBlank", {
    vertices: [
      { key: "lowerLeft", position: [-130, -70] },
      { key: "lowerRight", position: [-10, -70] },
      { key: "upperRight", position: [-10, 70] },
      { key: "upperLeft", position: [-130, 70] },
    ],
    closed: true,
  });
  const femaleBlankAnchor = $.constraint.fixedPoint("femaleBlankAnchor", {
    point: femaleBlank.vertices.byKey.lowerLeft,
    target: [-130, -70],
  });
  const femaleBlankWidth = $.dimension.curveLength("femaleBlankWidth", {
    curve: femaleBlank.segments.byKey.lowerLeft,
    value: mm(120),
  });
  const femaleBlankHeight = $.dimension.curveLength("femaleBlankHeight", {
    curve: femaleBlank.segments.byKey.upperLeft,
    value: mm(140),
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
  const looseMortiseBottom = $.constraint.horizontal("looseMortiseBottom", { span: looseMortise.segments.byKey.lowerLeft });
  const looseMortiseRight = $.constraint.vertical("looseMortiseRight", { span: looseMortise.segments.byKey.lowerRight });
  const looseMortiseTop = $.constraint.horizontal("looseMortiseTop", { span: looseMortise.segments.byKey.upperRight });
  const looseMortiseLeft = $.constraint.vertical("looseMortiseLeft", { span: looseMortise.segments.byKey.upperLeft });
  const looseMortiseWidth = $.dimension.curveLength("looseMortiseWidth", { curve: looseMortise.segments.byKey.lowerLeft, value: mm(70) });
  const looseMortiseHeight = $.dimension.curveLength("looseMortiseHeight", { curve: looseMortise.segments.byKey.lowerRight, value: mm(18.4) });

  const nominalMortise = $.geometry.polyline("nominalMortise", {
    vertices: [
      { key: "lowerLeft", position: [-105, -9] },
      { key: "lowerRight", position: [-35, -9] },
      { key: "upperRight", position: [-35, 9] },
      { key: "upperLeft", position: [-105, 9] },
    ],
    closed: true,
  });
  const nominalMortiseBottom = $.constraint.horizontal("nominalMortiseBottom", { span: nominalMortise.segments.byKey.lowerLeft });
  const nominalMortiseRight = $.constraint.vertical("nominalMortiseRight", { span: nominalMortise.segments.byKey.lowerRight });
  const nominalMortiseTop = $.constraint.horizontal("nominalMortiseTop", { span: nominalMortise.segments.byKey.upperRight });
  const nominalMortiseLeft = $.constraint.vertical("nominalMortiseLeft", { span: nominalMortise.segments.byKey.upperLeft });
  const nominalMortiseWidth = $.dimension.curveLength("nominalMortiseWidth", { curve: nominalMortise.segments.byKey.lowerLeft, value: mm(70) });
  const nominalMortiseHeight = $.dimension.curveLength("nominalMortiseHeight", { curve: nominalMortise.segments.byKey.lowerRight, value: mm(18) });

  const pressMortise = $.geometry.polyline("pressMortise", {
    vertices: [
      { key: "lowerLeft", position: [-105, -50.8] },
      { key: "lowerRight", position: [-35, -50.8] },
      { key: "upperRight", position: [-35, -33.2] },
      { key: "upperLeft", position: [-105, -33.2] },
    ],
    closed: true,
  });
  const pressMortiseBottom = $.constraint.horizontal("pressMortiseBottom", { span: pressMortise.segments.byKey.lowerLeft });
  const pressMortiseRight = $.constraint.vertical("pressMortiseRight", { span: pressMortise.segments.byKey.lowerRight });
  const pressMortiseTop = $.constraint.horizontal("pressMortiseTop", { span: pressMortise.segments.byKey.upperRight });
  const pressMortiseLeft = $.constraint.vertical("pressMortiseLeft", { span: pressMortise.segments.byKey.upperLeft });
  const pressMortiseWidth = $.dimension.curveLength("pressMortiseWidth", { curve: pressMortise.segments.byKey.lowerLeft, value: mm(70) });
  const pressMortiseHeight = $.dimension.curveLength("pressMortiseHeight", { curve: pressMortise.segments.byKey.lowerRight, value: mm(17.6) });

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

  const looseTab = $.geometry.polyline("looseTab", {
    vertices: [
      { key: "lowerLeft", position: [20, 33] },
      { key: "lowerRight", position: [115, 33] },
      { key: "upperRight", position: [115, 51] },
      { key: "upperLeft", position: [20, 51] },
    ],
    closed: true,
  });
  const nominalTab = $.geometry.polyline("nominalTab", {
    vertices: [
      { key: "lowerLeft", position: [20, -9] },
      { key: "lowerRight", position: [115, -9] },
      { key: "upperRight", position: [115, 9] },
      { key: "upperLeft", position: [20, 9] },
    ],
    closed: true,
  });
  const pressTab = $.geometry.polyline("pressTab", {
    vertices: [
      { key: "lowerLeft", position: [20, -51] },
      { key: "lowerRight", position: [115, -51] },
      { key: "upperRight", position: [115, -33] },
      { key: "upperLeft", position: [20, -33] },
    ],
    closed: true,
  });
  const looseTabLength = $.dimension.curveLength("looseTabLength", { curve: looseTab.segments.byKey.lowerLeft, value: mm(95) });
  const nominalTabLength = $.dimension.curveLength("nominalTabLength", { curve: nominalTab.segments.byKey.lowerLeft, value: mm(95) });
  const pressTabLength = $.dimension.curveLength("pressTabLength", { curve: pressTab.segments.byKey.lowerLeft, value: mm(95) });
  const looseTabThickness = $.dimension.curveLength("looseTabThickness", { curve: looseTab.segments.byKey.lowerRight, value: mm(18) });
  const nominalTabThickness = $.dimension.curveLength("nominalTabThickness", { curve: nominalTab.segments.byKey.lowerRight, value: mm(18) });
  const pressTabThickness = $.dimension.curveLength("pressTabThickness", { curve: pressTab.segments.byKey.lowerRight, value: mm(18) });

  // One absolute point plus relational construction datums place every fit
  // station. Literal positions above are only solver seeds, never authority.
  const blankToStationX = $.geometry.segment("blankToStationX", {
    start: femaleBlank.vertices.byKey.lowerLeft,
    end: [-105, -70],
    role: "construction",
  });
  const blankToStationXHorizontal = $.constraint.horizontal("blankToStationXHorizontal", { span: blankToStationX.span });
  const blankToStationXLength = $.dimension.curveLength("blankToStationXLength", { curve: blankToStationX.span, value: mm(25) });
  const pressMortiseY = $.geometry.segment("pressMortiseY", {
    start: blankToStationX.end,
    end: pressMortise.vertices.byKey.lowerLeft,
    role: "construction",
  });
  const pressMortiseYVertical = $.constraint.vertical("pressMortiseYVertical", { span: pressMortiseY.span });
  const pressMortiseYLength = $.dimension.curveLength("pressMortiseYLength", { curve: pressMortiseY.span, value: mm(19.2) });
  const pressToNominalMortise = $.geometry.segment("pressToNominalMortise", {
    start: pressMortise.vertices.byKey.lowerLeft,
    end: nominalMortise.vertices.byKey.lowerLeft,
    role: "construction",
  });
  const pressToNominalMortiseVertical = $.constraint.vertical("pressToNominalMortiseVertical", { span: pressToNominalMortise.span });
  const pressToNominalMortiseLength = $.dimension.curveLength("pressToNominalMortiseLength", { curve: pressToNominalMortise.span, value: mm(41.8) });
  const nominalToLooseMortise = $.geometry.segment("nominalToLooseMortise", {
    start: nominalMortise.vertices.byKey.lowerLeft,
    end: looseMortise.vertices.byKey.lowerLeft,
    role: "construction",
  });
  const nominalToLooseMortiseVertical = $.constraint.vertical("nominalToLooseMortiseVertical", { span: nominalToLooseMortise.span });
  const nominalToLooseMortiseLength = $.dimension.curveLength("nominalToLooseMortiseLength", { curve: nominalToLooseMortise.span, value: mm(41.8) });
  const nominalStationWidth = $.geometry.segment("nominalStationWidth", {
    start: nominalMortise.vertices.byKey.lowerLeft,
    end: nominalTab.vertices.byKey.lowerLeft,
    role: "construction",
  });
  const nominalStationWidthHorizontal = $.constraint.horizontal("nominalStationWidthHorizontal", { span: nominalStationWidth.span });
  const nominalStationWidthLength = $.dimension.curveLength("nominalStationWidthLength", { curve: nominalStationWidth.span, value: mm(125) });
  const pressToNominalTab = $.geometry.segment("pressToNominalTab", {
    start: pressTab.vertices.byKey.lowerLeft,
    end: nominalTab.vertices.byKey.lowerLeft,
    role: "construction",
  });
  const pressToNominalTabVertical = $.constraint.vertical("pressToNominalTabVertical", { span: pressToNominalTab.span });
  const pressToNominalTabLength = $.dimension.curveLength("pressToNominalTabLength", { curve: pressToNominalTab.span, value: mm(42) });
  const nominalToLooseTab = $.geometry.segment("nominalToLooseTab", {
    start: nominalTab.vertices.byKey.lowerLeft,
    end: looseTab.vertices.byKey.lowerLeft,
    role: "construction",
  });
  const nominalToLooseTabVertical = $.constraint.vertical("nominalToLooseTabVertical", { span: nominalToLooseTab.span });
  const nominalToLooseTabLength = $.dimension.curveLength("nominalToLooseTabLength", { curve: nominalToLooseTab.span, value: mm(42) });

  const edgeRadius = mm(6);
  const blankHandling = $.use("blankHandling", fillets, {
    corners: {
      lowerLeft: femaleBlank.filletableCorners.byKey.lowerLeft,
      lowerRight: femaleBlank.filletableCorners.byKey.lowerRight,
      upperRight: femaleBlank.filletableCorners.byKey.upperRight,
      upperLeft: femaleBlank.filletableCorners.byKey.upperLeft,
    },
    radius: edgeRadius,
  });
  const looseTabHandling = $.use("looseTabHandling", fillets, {
    corners: {
      lowerRight: looseTab.filletableCorners.byKey.lowerRight,
      upperRight: looseTab.filletableCorners.byKey.upperRight,
    },
    radius: edgeRadius,
  });
  const nominalTabHandling = $.use("nominalTabHandling", fillets, {
    corners: {
      lowerRight: nominalTab.filletableCorners.byKey.lowerRight,
      upperRight: nominalTab.filletableCorners.byKey.upperRight,
    },
    radius: edgeRadius,
  });
  const pressTabHandling = $.use("pressTabHandling", fillets, {
    corners: {
      lowerRight: pressTab.filletableCorners.byKey.lowerRight,
      upperRight: pressTab.filletableCorners.byKey.upperRight,
    },
    radius: edgeRadius,
  });

  $.group("Female coupon blank", [femaleBlank, femaleBlankAnchor, femaleBlankWidth, femaleBlankHeight]);
  $.group("Relational placement", [blankToStationX, pressMortiseY, pressToNominalMortise, nominalToLooseMortise, nominalStationWidth, pressToNominalTab, nominalToLooseTab]);
  $.group("Loose fit", [looseMortise, looseTab]);
  $.group("Nominal fit", [nominalMortise, nominalTab]);
  $.group("Press fit", [pressMortise, pressTab]);

  return {
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
  };
});
