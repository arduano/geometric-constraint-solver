"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // The 122 x 37 mm bounds and 116 mm inner span are published drawing
  // datums. Curved cutout geometry below is deliberately schematic: it makes
  // the central T-passage and paired edge-notch arrangement recognizable,
  // but is neither a traced profile nor a cut-ready replacement DXF.
  const motorPanel = $.operation.rectangle("motorPanel", {
    origin: [-61, -18.5],
    width: mm(122),
    height: mm(37),
    label: "Simplified V0.2 panel bounding envelope",
    role: "profile",
  });
  const innerSpanDatum = $.operation.rectangle("innerSpanDatum", {
    origin: [-58, -0.25],
    width: mm(116),
    height: mm(0.5),
    label: "Published 116 mm inner-span datum",
    role: "construction",
  });
  const centralPassageProxy = $.operation.slot("centralPassageProxy", {
    firstCenter: [0, -6.5],
    secondCenter: [0, 8],
    radius: mm(5),
    label: "Schematic proxy for central T-shaped passage",
    role: "profile",
  });
  const sideNotchDatum = $.operation.rectangle("sideNotchDatum", {
    origin: [-58, 2],
    width: mm(116),
    height: mm(0.5),
    label: "Schematic side-notch center datum",
    role: "construction",
  });
  const leftSideNotch = $.geometry.centerRadiusCircle("leftSideNotch", {
    center: [-58, 2],
    radius: mm(2),
    label: "Schematic left edge-notch reference",
    role: "construction",
  });
  const rightSideNotch = $.geometry.centerRadiusCircle("rightSideNotch", {
    center: [58, 2],
    radius: mm(2),
    label: "Schematic right edge-notch reference",
    role: "construction",
  });
  const locateLeftSideNotch = $.constraint.coincident("locateLeftSideNotch", {
    first: leftSideNotch.center,
    second: sideNotchDatum.corners.bottomLeft,
    label: "Locate schematic left side notch",
  });
  const locateRightSideNotch = $.constraint.coincident("locateRightSideNotch", {
    first: rightSideNotch.center,
    second: sideNotchDatum.corners.bottomRight,
    label: "Locate schematic right side notch",
  });
  const sideNotchRadius = $.dimension.radius("sideNotchRadius", {
    curve: leftSideNotch.curve,
    value: mm(2),
    label: "Schematic edge-notch reference radius",
    mode: "driving",
  });
  const matchingSideNotch = $.constraint.equalRadius("matchingSideNotch", {
    first: leftSideNotch.curve,
    second: rightSideNotch.curve,
    label: "Matched schematic side-notch references",
  });
  $.group("Panel envelope", [motorPanel, innerSpanDatum]);
  $.group("Central motor passage", [centralPassageProxy]);
  $.group("Side-notch references", [sideNotchDatum, leftSideNotch, rightSideNotch, locateLeftSideNotch, locateRightSideNotch, sideNotchRadius, matchingSideNotch]);
  return {};
});
