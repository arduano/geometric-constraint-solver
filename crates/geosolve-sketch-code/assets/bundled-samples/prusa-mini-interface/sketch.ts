"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // Selected bearing-normal interface from MINI-x-carriage.stp. STEP (X,Y)
  // becomes sketch (Y,X-17): R7.5 bearing seats at X=0 and 34.
  // The two small normal-axis interfaces are at STEP (1.75,12) and (46.5,4).
  // The independently centred STL bounding box is construction, not a traced part.
  // Pitch and radius edits explore an adapter variant; they do not change OEM data.
  const carriageEnvelope = $.operation.rectangle("carriageEnvelope", {
    origin: [-15.7115, -34.7035],
    width: mm(31.423),
    height: mm(69.407),
    label: "Published mesh bounds, centred for comparison",
    role: "construction",
  });
  const bearingAxis = $.geometry.segment("bearingAxis", {
    start: [0, -17],
    end: [0, 17],
    branchDirection: [0, 1],
    role: "construction",
    label: "Published bearing seat pitch axis",
  });
  const bearingAligned = $.constraint.vertical("bearingAligned", {
    span: bearingAxis.span,
    label: "Published bearing seat alignment",
  });
  const bearingCenter = $.geometry.sketchPoint("bearingCenter", {
    point: [0, 0],
    role: "construction",
    label: "Published bearing seat midpoint",
  });
  const bearingOrigin = $.constraint.fixedPoint("bearingOrigin", {
    point: bearingCenter.point,
    target: [0, 0],
    label: "Published bearing seat reference midpoint",
  });
  const bearingCentered = $.constraint.midpoint("bearingCentered", {
    point: bearingCenter.point,
    line: bearingAxis.span,
    label: "Published bearing seat symmetric pitch",
  });
  const bearingPitch = $.dimension.curveLength("bearingPitch", {
    curve: bearingAxis.span,
    value: mm(34),
    mode: "driving",
    label: "Published bearing seat centre distance",
  });
  const bearingFirst = $.geometry.centerRadiusCircle("bearingFirst", {
    center: bearingAxis.start,
    radius: mm(7.5),
    role: "profile",
    label: "Published bearing seat first",
  });
  const bearingSecond = $.geometry.centerRadiusCircle("bearingSecond", {
    center: bearingAxis.end,
    radius: mm(7.5),
    role: "profile",
    label: "Published bearing seat second",
  });
  const bearingRadius = $.dimension.radius("bearingRadius", {
    curve: bearingFirst.curve,
    value: mm(7.5),
    mode: "driving",
    label: "Published bearing seat radius",
  });
  const bearingMatched = $.constraint.equalRadius("bearingMatched", {
    first: bearingFirst.curve,
    second: bearingSecond.curve,
    label: "Published bearing seat matching radii",
  });
  const lowerInterface = $.geometry.centerRadiusCircle("lowerInterface", {
    center: [12, -15.25],
    radius: mm(1.6),
    role: "profile",
    label: "STEP lowerInterface normal-axis interface",
  });
  const lowerInterfaceX = $.constraint.fixedCoordinate("lowerInterfaceX", {
    point: lowerInterface.center,
    axis: "x",
    target: mm(12),
    label: "lowerInterface measured x offset",
  });
  const lowerInterfaceY = $.constraint.fixedCoordinate("lowerInterfaceY", {
    point: lowerInterface.center,
    axis: "y",
    target: mm(-15.25),
    label: "lowerInterface measured y offset",
  });
  const lowerInterfaceRadius = $.dimension.radius("lowerInterfaceRadius", {
    curve: lowerInterface.curve,
    value: mm(1.6),
    mode: "driving",
    label: "lowerInterface measured bore radius",
  });
  const upperInterface = $.geometry.centerRadiusCircle("upperInterface", {
    center: [4, 29.5],
    radius: mm(1.65),
    role: "profile",
    label: "STEP upperInterface normal-axis interface",
  });
  const upperInterfaceX = $.constraint.fixedCoordinate("upperInterfaceX", {
    point: upperInterface.center,
    axis: "x",
    target: mm(4),
    label: "upperInterface measured x offset",
  });
  const upperInterfaceY = $.constraint.fixedCoordinate("upperInterfaceY", {
    point: upperInterface.center,
    axis: "y",
    target: mm(29.5),
    label: "upperInterface measured y offset",
  });
  const upperInterfaceRadius = $.dimension.radius("upperInterfaceRadius", {
    curve: upperInterface.curve,
    value: mm(1.65),
    mode: "driving",
    label: "upperInterface measured bore radius",
  });
  $.group("Published X-carriage envelope", [carriageEnvelope]);
  $.group("Bearing seats", [bearingAxis, bearingAligned, bearingCenter, bearingOrigin, bearingCentered, bearingPitch, bearingFirst, bearingSecond, bearingRadius, bearingMatched]);
  $.group("Selected mounting interface", [lowerInterface, lowerInterfaceX, lowerInterfaceY, lowerInterfaceRadius, upperInterface, upperInterfaceX, upperInterfaceY, upperInterfaceRadius]);
  return {};
});
