"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch({
  title: "CNC joinery and dogbone fit coupon",
  description: "A fully constrained three-station router coupon compares press, nominal and loose mortises against common tabs and explicit cutter reliefs.",
}, ($) => {
  // Sacrificial router-fit coupon comparing loose, nominal and press stations.
  // The corner-centred circles are explicit dogbone overcuts, not CAM output.
  const femaleBlank = $.geometry.twoPointAlignedRectangle("femaleBlank", {
    firstCorner: [-130, -70],
    oppositeCorner: [-10, 70],
    label: "Female coupon blank",
    role: "profile",
  });
  const femaleBlankAnchor = $.constraint.fixedPoint("femaleBlankAnchor", {
    point: femaleBlank.corners[0],
    target: [-130, -70],
    label: "Coupon datum",
  });
  const femaleBlankWidth = $.dimension.curveLength("femaleBlankWidth", {
    curve: femaleBlank.spans[0],
    value: mm(120),
    label: "Female blank width",
    mode: "driving",
  });
  const femaleBlankHeight = $.dimension.curveLength("femaleBlankHeight", {
    curve: femaleBlank.spans[1],
    value: mm(140),
    label: "Female blank height",
    mode: "driving",
  });
  const stationX = $.geometry.segment("stationX", {
    start: femaleBlank.corners[0],
    end: [-105, -70],
    branchDirection: [1, 0],
    label: "Mortise station X datum",
    role: "construction",
  });
  const stationXHorizontal = $.constraint.horizontal("stationXHorizontal", {
    span: stationX.span,
    label: "Station X axis",
  });
  const stationXInset = $.dimension.curveLength("stationXInset", {
    curve: stationX.span,
    value: mm(25),
    label: "Mortise X inset",
    mode: "driving",
  });
  const pressMortiseY = $.geometry.segment("pressMortiseY", {
    start: stationX.end,
    end: [-105, -50.8],
    branchDirection: [0, 1],
    label: "Press station Y datum",
    role: "construction",
  });
  const pressMortiseYVertical = $.constraint.vertical("pressMortiseYVertical", {
    span: pressMortiseY.span,
    label: "Press station vertical datum",
  });
  const pressMortiseYInset = $.dimension.curveLength("pressMortiseYInset", {
    curve: pressMortiseY.span,
    value: mm(19.2),
    label: "Press station lower inset",
    mode: "driving",
  });
  const pressToNominal = $.geometry.segment("pressToNominal", {
    start: pressMortiseY.end,
    end: [-105, -9],
    branchDirection: [0, 1],
    label: "Press-to-nominal pitch",
    role: "construction",
  });
  const pressToNominalVertical = $.constraint.vertical("pressToNominalVertical", {
    span: pressToNominal.span,
    label: "Mortise pitch axis",
  });
  const pressToNominalPitch = $.dimension.curveLength("pressToNominalPitch", {
    curve: pressToNominal.span,
    value: mm(41.8),
    label: "Press-to-nominal pitch",
    mode: "driving",
  });
  const nominalToLoose = $.geometry.segment("nominalToLoose", {
    start: pressToNominal.end,
    end: [-105, 32.8],
    branchDirection: [0, 1],
    label: "Nominal-to-loose pitch",
    role: "construction",
  });
  const nominalToLooseVertical = $.constraint.vertical("nominalToLooseVertical", {
    span: nominalToLoose.span,
    label: "Mortise pitch axis",
  });
  const nominalToLoosePitch = $.dimension.curveLength("nominalToLoosePitch", {
    curve: nominalToLoose.span,
    value: mm(41.8),
    label: "Nominal-to-loose pitch",
    mode: "driving",
  });
  const pressMortise = $.geometry.twoPointAlignedRectangle("pressMortise", {
    firstCorner: pressMortiseY.end,
    oppositeCorner: [-35, -33.2],
    label: "17.6 mm press mortise",
    role: "profile",
  });
  const pressMortiseWidth = $.dimension.curveLength("pressMortiseWidth", {
    curve: pressMortise.spans[0],
    value: mm(70),
    label: "Press mortise length",
    mode: "driving",
  });
  const pressMortiseHeight = $.dimension.curveLength("pressMortiseHeight", {
    isKeyConstraint: true,
    curve: pressMortise.spans[1],
    value: mm(17.6),
    label: "Press mortise width",
    mode: "driving",
  });
  const nominalMortise = $.geometry.twoPointAlignedRectangle("nominalMortise", {
    firstCorner: pressToNominal.end,
    oppositeCorner: [-35, 9],
    label: "18.0 mm nominal mortise",
    role: "profile",
  });
  const nominalMortiseWidth = $.dimension.curveLength("nominalMortiseWidth", {
    curve: nominalMortise.spans[0],
    value: mm(70),
    label: "Nominal mortise length",
    mode: "driving",
  });
  const nominalMortiseHeight = $.dimension.curveLength("nominalMortiseHeight", {
    isKeyConstraint: true,
    curve: nominalMortise.spans[1],
    value: mm(18),
    label: "Nominal mortise width",
    mode: "driving",
  });
  const looseMortise = $.geometry.twoPointAlignedRectangle("looseMortise", {
    firstCorner: nominalToLoose.end,
    oppositeCorner: [-35, 51.2],
    label: "18.4 mm loose mortise",
    role: "profile",
  });
  const looseMortiseWidth = $.dimension.curveLength("looseMortiseWidth", {
    curve: looseMortise.spans[0],
    value: mm(70),
    label: "Loose mortise length",
    mode: "driving",
  });
  const looseMortiseHeight = $.dimension.curveLength("looseMortiseHeight", {
    isKeyConstraint: true,
    curve: looseMortise.spans[1],
    value: mm(18.4),
    label: "Loose mortise width",
    mode: "driving",
  });
  const nominalTabDatum = $.geometry.segment("nominalTabDatum", {
    start: nominalMortise.corners[0],
    end: [20, -9],
    branchDirection: [1, 0],
    label: "Male coupon station datum",
    role: "construction",
  });
  const nominalTabDatumHorizontal = $.constraint.horizontal("nominalTabDatumHorizontal", {
    span: nominalTabDatum.span,
    label: "Male station axis",
  });
  const stationGap = $.dimension.curveLength("stationGap", {
    curve: nominalTabDatum.span,
    value: mm(125),
    label: "Female-to-male station gap",
    mode: "driving",
  });
  const pressTabDatum = $.geometry.segment("pressTabDatum", {
    start: nominalTabDatum.end,
    end: [20, -51],
    branchDirection: [0, -1],
    label: "Press tab pitch",
    role: "construction",
  });
  const pressTabDatumVertical = $.constraint.vertical("pressTabDatumVertical", {
    span: pressTabDatum.span,
    label: "Male pitch axis",
  });
  const pressTabPitch = $.dimension.curveLength("pressTabPitch", {
    curve: pressTabDatum.span,
    value: mm(42),
    label: "Press tab pitch",
    mode: "driving",
  });
  const looseTabDatum = $.geometry.segment("looseTabDatum", {
    start: nominalTabDatum.end,
    end: [20, 33],
    branchDirection: [0, 1],
    label: "Loose tab pitch",
    role: "construction",
  });
  const looseTabDatumVertical = $.constraint.vertical("looseTabDatumVertical", {
    span: looseTabDatum.span,
    label: "Male pitch axis",
  });
  const looseTabPitch = $.dimension.curveLength("looseTabPitch", {
    curve: looseTabDatum.span,
    value: mm(42),
    label: "Loose tab pitch",
    mode: "driving",
  });
  const pressTab = $.geometry.twoPointAlignedRectangle("pressTab", {
    firstCorner: pressTabDatum.end,
    oppositeCorner: [115, -33],
    label: "Press-fit test tab",
    role: "profile",
  });
  const pressTabLength = $.dimension.curveLength("pressTabLength", {
    curve: pressTab.spans[0],
    value: mm(95),
    label: "Press tab length",
    mode: "driving",
  });
  const pressTabThickness = $.dimension.curveLength("pressTabThickness", {
    curve: pressTab.spans[1],
    value: mm(18),
    label: "Press tab thickness",
    mode: "driving",
  });
  const nominalTab = $.geometry.twoPointAlignedRectangle("nominalTab", {
    firstCorner: nominalTabDatum.end,
    oppositeCorner: [115, 9],
    label: "Nominal test tab",
    role: "profile",
  });
  const nominalTabLength = $.dimension.curveLength("nominalTabLength", {
    curve: nominalTab.spans[0],
    value: mm(95),
    label: "Nominal tab length",
    mode: "driving",
  });
  const nominalTabThickness = $.dimension.curveLength("nominalTabThickness", {
    isKeyConstraint: true,
    curve: nominalTab.spans[1],
    value: mm(18),
    label: "Nominal tab thickness",
    mode: "driving",
  });
  const looseTab = $.geometry.twoPointAlignedRectangle("looseTab", {
    firstCorner: looseTabDatum.end,
    oppositeCorner: [115, 51],
    label: "Loose-fit test tab",
    role: "profile",
  });
  const looseTabLength = $.dimension.curveLength("looseTabLength", {
    curve: looseTab.spans[0],
    value: mm(95),
    label: "Loose tab length",
    mode: "driving",
  });
  const looseTabThickness = $.dimension.curveLength("looseTabThickness", {
    curve: looseTab.spans[1],
    value: mm(18),
    label: "Loose tab thickness",
    mode: "driving",
  });
  const pressReliefLl = $.geometry.centerRadiusCircle("pressReliefLl", {
    center: pressMortise.corners[0],
    radius: mm(3.175),
    label: "Press LL cutter relief",
    role: "profile",
  });
  const pressReliefLr = $.geometry.centerRadiusCircle("pressReliefLr", {
    center: pressMortise.corners[1],
    radius: mm(3.175),
    label: "Press LR cutter relief",
    role: "profile",
  });
  const pressReliefUr = $.geometry.centerRadiusCircle("pressReliefUr", {
    center: pressMortise.corners[2],
    radius: mm(3.175),
    label: "Press UR cutter relief",
    role: "profile",
  });
  const pressReliefUl = $.geometry.centerRadiusCircle("pressReliefUl", {
    center: pressMortise.corners[3],
    radius: mm(3.175),
    label: "Press UL cutter relief",
    role: "profile",
  });
  const nominalReliefLl = $.geometry.centerRadiusCircle("nominalReliefLl", {
    center: nominalMortise.corners[0],
    radius: mm(3.175),
    label: "Nominal LL cutter relief",
    role: "profile",
  });
  const nominalReliefLr = $.geometry.centerRadiusCircle("nominalReliefLr", {
    center: nominalMortise.corners[1],
    radius: mm(3.175),
    label: "Nominal LR cutter relief",
    role: "profile",
  });
  const nominalReliefUr = $.geometry.centerRadiusCircle("nominalReliefUr", {
    center: nominalMortise.corners[2],
    radius: mm(3.175),
    label: "Nominal UR cutter relief",
    role: "profile",
  });
  const nominalReliefUl = $.geometry.centerRadiusCircle("nominalReliefUl", {
    center: nominalMortise.corners[3],
    radius: mm(3.175),
    label: "Nominal UL cutter relief",
    role: "profile",
  });
  const looseReliefLl = $.geometry.centerRadiusCircle("looseReliefLl", {
    center: looseMortise.corners[0],
    radius: mm(3.175),
    label: "Loose LL cutter relief",
    role: "profile",
  });
  const looseReliefLr = $.geometry.centerRadiusCircle("looseReliefLr", {
    center: looseMortise.corners[1],
    radius: mm(3.175),
    label: "Loose LR cutter relief",
    role: "profile",
  });
  const looseReliefUr = $.geometry.centerRadiusCircle("looseReliefUr", {
    center: looseMortise.corners[2],
    radius: mm(3.175),
    label: "Loose UR cutter relief",
    role: "profile",
  });
  const looseReliefUl = $.geometry.centerRadiusCircle("looseReliefUl", {
    center: looseMortise.corners[3],
    radius: mm(3.175),
    label: "Loose UL cutter relief",
    role: "profile",
  });
  const cutterRadius = $.dimension.radius("cutterRadius", {
    isKeyConstraint: true,
    curve: pressReliefLl.curve,
    value: mm(3.175),
    label: "6.35 mm cutter",
    mode: "driving",
  });
  const pressReliefLrEqual = $.constraint.equalRadius("pressReliefLrEqual", {
    first: pressReliefLl.curve,
    second: pressReliefLr.curve,
    label: "Common cutter radius",
  });
  const pressReliefUrEqual = $.constraint.equalRadius("pressReliefUrEqual", {
    first: pressReliefLl.curve,
    second: pressReliefUr.curve,
    label: "Common cutter radius",
  });
  const pressReliefUlEqual = $.constraint.equalRadius("pressReliefUlEqual", {
    first: pressReliefLl.curve,
    second: pressReliefUl.curve,
    label: "Common cutter radius",
  });
  const nominalReliefLlEqual = $.constraint.equalRadius("nominalReliefLlEqual", {
    first: pressReliefLl.curve,
    second: nominalReliefLl.curve,
    label: "Common cutter radius",
  });
  const nominalReliefLrEqual = $.constraint.equalRadius("nominalReliefLrEqual", {
    first: pressReliefLl.curve,
    second: nominalReliefLr.curve,
    label: "Common cutter radius",
  });
  const nominalReliefUrEqual = $.constraint.equalRadius("nominalReliefUrEqual", {
    first: pressReliefLl.curve,
    second: nominalReliefUr.curve,
    label: "Common cutter radius",
  });
  const nominalReliefUlEqual = $.constraint.equalRadius("nominalReliefUlEqual", {
    first: pressReliefLl.curve,
    second: nominalReliefUl.curve,
    label: "Common cutter radius",
  });
  const looseReliefLlEqual = $.constraint.equalRadius("looseReliefLlEqual", {
    first: pressReliefLl.curve,
    second: looseReliefLl.curve,
    label: "Common cutter radius",
  });
  const looseReliefLrEqual = $.constraint.equalRadius("looseReliefLrEqual", {
    first: pressReliefLl.curve,
    second: looseReliefLr.curve,
    label: "Common cutter radius",
  });
  const looseReliefUrEqual = $.constraint.equalRadius("looseReliefUrEqual", {
    first: pressReliefLl.curve,
    second: looseReliefUr.curve,
    label: "Common cutter radius",
  });
  const looseReliefUlEqual = $.constraint.equalRadius("looseReliefUlEqual", {
    first: pressReliefLl.curve,
    second: looseReliefUl.curve,
    label: "Common cutter radius",
  });
  $.group("Female coupon blank", [femaleBlank, femaleBlankAnchor, femaleBlankWidth, femaleBlankHeight]);
  $.group("Relational station datums", [stationX, stationXHorizontal, stationXInset, pressMortiseY, pressMortiseYVertical, pressMortiseYInset, pressToNominal, pressToNominalVertical, pressToNominalPitch, nominalToLoose, nominalToLooseVertical, nominalToLoosePitch, nominalTabDatum, nominalTabDatumHorizontal, stationGap, pressTabDatum, pressTabDatumVertical, pressTabPitch, looseTabDatum, looseTabDatumVertical, looseTabPitch]);
  $.group("Press-fit station", [pressMortise, pressMortiseWidth, pressMortiseHeight, pressTab, pressTabLength, pressTabThickness, pressReliefLl, pressReliefLr, pressReliefUr, pressReliefUl, cutterRadius, pressReliefLrEqual, pressReliefUrEqual, pressReliefUlEqual]);
  $.group("Nominal-fit station", [nominalMortise, nominalMortiseWidth, nominalMortiseHeight, nominalTab, nominalTabLength, nominalTabThickness, nominalReliefLl, nominalReliefLr, nominalReliefUr, nominalReliefUl, nominalReliefLlEqual, nominalReliefLrEqual, nominalReliefUrEqual, nominalReliefUlEqual]);
  $.group("Loose-fit station", [looseMortise, looseMortiseWidth, looseMortiseHeight, looseTab, looseTabLength, looseTabThickness, looseReliefLl, looseReliefLr, looseReliefUr, looseReliefUl, looseReliefLlEqual, looseReliefLrEqual, looseReliefUrEqual, looseReliefUlEqual]);
  return {};
});
