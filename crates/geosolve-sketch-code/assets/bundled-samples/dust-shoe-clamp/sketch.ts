"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // A planar router dust-shoe interface: a split spindle ring feeds a second
  // extraction port and a symmetric two-screw clamp lug. All locations are
  // related to one origin datum rather than frozen independently.
  const spindleAxis = $.geometry.sketchPoint("spindleAxis", {
    point: [0, 0],
    label: "Spindle axis",
    role: "construction",
  });
  const spindleDatum = $.constraint.coincidentWithOrigin("spindleDatum", {
    point: spindleAxis.point,
    label: "Machine datum",
  });
  const spindleBore = $.geometry.centerRadiusCircle("spindleBore", {
    center: spindleAxis.point,
    radius: mm(32.5),
    label: "65 mm spindle bore",
    role: "profile",
  });
  const clampOutside = $.geometry.centerRadiusCircle("clampOutside", {
    center: spindleAxis.point,
    radius: mm(42),
    label: "Clamp outside",
    role: "profile",
  });
  const spindleBoreRadius = $.dimension.radius("spindleBoreRadius", {
    curve: spindleBore.curve,
    value: mm(32.5),
    label: "Spindle bore radius",
    mode: "driving",
  });
  const clampOutsideRadius = $.dimension.radius("clampOutsideRadius", {
    curve: clampOutside.curve,
    value: mm(42),
    label: "Clamp outside radius",
    mode: "driving",
  });
  const dustPortDatum = $.geometry.segment("dustPortDatum", {
    start: spindleAxis.point,
    end: [0, 55],
    branchDirection: [0, 1],
    label: "Extraction-port centreline",
    role: "construction",
  });
  const dustPortAxis = $.constraint.vertical("dustPortAxis", {
    span: dustPortDatum.span,
    label: "Port vertical datum",
  });
  const dustPortOffset = $.dimension.curveLength("dustPortOffset", {
    curve: dustPortDatum.span,
    value: mm(55),
    label: "Port offset",
    mode: "driving",
  });
  const dustPortBore = $.geometry.centerRadiusCircle("dustPortBore", {
    center: dustPortDatum.end,
    radius: mm(14),
    label: "28 mm hose bore",
    role: "profile",
  });
  const dustPortOutside = $.geometry.centerRadiusCircle("dustPortOutside", {
    center: dustPortDatum.end,
    radius: mm(18),
    label: "Extraction boss outside",
    role: "profile",
  });
  const dustPortBoreRadius = $.dimension.radius("dustPortBoreRadius", {
    curve: dustPortBore.curve,
    value: mm(14),
    label: "Dust port bore radius",
    mode: "driving",
  });
  const dustPortOutsideRadius = $.dimension.radius("dustPortOutsideRadius", {
    curve: dustPortOutside.curve,
    value: mm(18),
    label: "Dust port outside radius",
    mode: "driving",
  });
  const lugCenterline = $.geometry.segment("lugCenterline", {
    start: spindleAxis.point,
    end: [-52, 0],
    branchDirection: [-1, 0],
    label: "Clamp-lug centreline",
    role: "construction",
  });
  const lugAxis = $.constraint.horizontal("lugAxis", {
    span: lugCenterline.span,
    label: "Lug horizontal datum",
  });
  const lugOffset = $.dimension.curveLength("lugOffset", {
    curve: lugCenterline.span,
    value: mm(52),
    label: "Lug centre offset",
    mode: "driving",
  });
  const clampLug = $.geometry.twoPointAlignedRectangle("clampLug", {
    firstCorner: [-64, -18],
    oppositeCorner: [-40, 18],
    label: "Clamp lug",
    role: "profile",
  });
  const lugDiagonal = $.geometry.segment("lugDiagonal", {
    start: clampLug.corners[0],
    end: clampLug.corners[2],
    branchDirection: [0.5547001962252291, 0.8320502943378437],
    label: "Lug-centre diagonal",
    role: "construction",
  });
  const lugCentered = $.constraint.midpoint("lugCentered", {
    point: lugCenterline.end,
    line: lugDiagonal.span,
    label: "Lug centered on datum",
  });
  const lugWidth = $.dimension.curveLength("lugWidth", {
    curve: clampLug.spans[0],
    value: mm(24),
    label: "Lug width",
    mode: "driving",
  });
  const lugHeight = $.dimension.curveLength("lugHeight", {
    curve: clampLug.spans[1],
    value: mm(36),
    label: "Lug height",
    mode: "driving",
  });
  const screwAxis = $.geometry.segment("screwAxis", {
    start: [-52, -10],
    end: [-52, 10],
    branchDirection: [0, 1],
    label: "Clamp-screw pitch line",
    role: "construction",
  });
  const screwAxisVertical = $.constraint.vertical("screwAxisVertical", {
    span: screwAxis.span,
    label: "Vertical screw axis",
  });
  const screwAxisLength = $.dimension.curveLength("screwAxisLength", {
    curve: screwAxis.span,
    value: mm(20),
    label: "Screw pitch",
    mode: "driving",
  });
  const screwAxisCentered = $.constraint.midpoint("screwAxisCentered", {
    point: lugCenterline.end,
    line: screwAxis.span,
    label: "Screw pair centered on lug",
  });
  const lowerClampScrew = $.geometry.centerRadiusCircle("lowerClampScrew", {
    center: screwAxis.start,
    radius: mm(2.6),
    label: "Lower M5 clearance",
    role: "profile",
  });
  const upperClampScrew = $.geometry.centerRadiusCircle("upperClampScrew", {
    center: screwAxis.end,
    radius: mm(2.6),
    label: "Upper M5 clearance",
    role: "profile",
  });
  const lowerClampScrewRadius = $.dimension.radius("lowerClampScrewRadius", {
    curve: lowerClampScrew.curve,
    value: mm(2.6),
    label: "Lower screw radius",
    mode: "driving",
  });
  const upperClampScrewRadius = $.dimension.radius("upperClampScrewRadius", {
    curve: upperClampScrew.curve,
    value: mm(2.6),
    label: "Upper screw radius",
    mode: "driving",
  });
  // The split crosses the screw lug and overlaps the bore by 1.5 mm.
  // Its two sides can be drawn together by the screw pair.
  const splitRelief = $.geometry.twoPointAlignedRectangle("splitRelief", {
    firstCorner: [-66, -2.5],
    oppositeCorner: [-31, 2.5],
    label: "Clamp split relief",
    role: "profile",
  });
  const splitX = $.constraint.fixedCoordinate("splitX", {
    point: splitRelief.corners[0],
    axis: "x",
    target: mm(-66),
    label: "Split begins outside the clamp lug",
  });
  const splitY = $.constraint.fixedCoordinate("splitY", {
    point: splitRelief.corners[0],
    axis: "y",
    target: mm(-2.5),
    label: "Split lower datum",
  });
  const splitLength = $.dimension.curveLength("splitLength", {
    curve: splitRelief.spans[0],
    value: mm(35),
    label: "Split length",
    mode: "driving",
  });
  const splitGap = $.dimension.curveLength("splitGap", {
    curve: splitRelief.spans[1],
    value: mm(5),
    label: "Split gap",
    mode: "driving",
  });
  $.group("Spindle clamp ring", [spindleAxis, spindleDatum, spindleBore, clampOutside, spindleBoreRadius, clampOutsideRadius]);
  $.group("Dust extraction port", [dustPortDatum, dustPortAxis, dustPortOffset, dustPortBore, dustPortOutside, dustPortBoreRadius, dustPortOutsideRadius]);
  $.group("Symmetric clamp lug", [lugCenterline, lugAxis, lugOffset, clampLug, lugDiagonal, lugCentered, lugWidth, lugHeight, screwAxis, screwAxisVertical, screwAxisLength, screwAxisCentered, lowerClampScrew, upperClampScrew, lowerClampScrewRadius, upperClampScrewRadius]);
  $.group("Split relief", [splitRelief, splitX, splitY, splitLength, splitGap]);
  return {};
});
