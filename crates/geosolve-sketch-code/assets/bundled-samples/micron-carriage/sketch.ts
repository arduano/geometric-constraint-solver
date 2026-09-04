"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // Independently redrawn planar interface study. The body and wing are
  // diagrammatic; the MGN9H 15 x 20 mm fastener pitch and selected toolhead
  // relationships are exposed without reproducing the STEP assembly.
  const carriageBody = $.operation.rectangle("carriageBody", {
    origin: [-30, -22],
    width: mm(60),
    height: mm(44),
    label: "Schematic Micron CNC carriage body",
    role: "profile",
  });
  const upperWing = $.operation.rectangle("upperWing", {
    origin: [-42, 12],
    width: mm(84),
    height: mm(10),
    label: "Schematic upper belt-clamp wing",
    role: "profile",
  });
  const railDatum = $.operation.rectangle("railDatum", {
    origin: [-7.5, -10],
    width: mm(15),
    height: mm(20),
    label: "MGN9H 15 x 20 mounting-pitch datum",
    role: "construction",
  });
  const centerline = $.operation.rectangle("centerline", {
    origin: [-0.1, -32],
    width: mm(0.2),
    height: mm(64),
    label: "Schematic 0.2 mm centerline band",
    role: "construction",
  });
  const railLowerLeft = $.geometry.centerRadiusCircle("railLowerLeft", {
    center: [-7.5, -10],
    radius: mm(1.6),
    label: "Rail lower-left M3 clearance",
    role: "profile",
  });
  const railLowerRight = $.geometry.centerRadiusCircle("railLowerRight", {
    center: [7.5, -10],
    radius: mm(1.6),
    label: "Rail lower-right M3 clearance",
    role: "profile",
  });
  const railUpperRight = $.geometry.centerRadiusCircle("railUpperRight", {
    center: [7.5, 10],
    radius: mm(1.6),
    label: "Rail upper-right M3 clearance",
    role: "profile",
  });
  const railUpperLeft = $.geometry.centerRadiusCircle("railUpperLeft", {
    center: [-7.5, 10],
    radius: mm(1.6),
    label: "Rail upper-left M3 clearance",
    role: "profile",
  });
  const railLowerLeftOnPitch = $.constraint.coincident("railLowerLeftOnPitch", {
    first: railLowerLeft.center,
    second: railDatum.corners.bottomLeft,
    label: "Locate rail lower-left mount",
  });
  const railLowerRightOnPitch = $.constraint.coincident("railLowerRightOnPitch", {
    first: railLowerRight.center,
    second: railDatum.corners.bottomRight,
    label: "Locate rail lower-right mount",
  });
  const railUpperRightOnPitch = $.constraint.coincident("railUpperRightOnPitch", {
    first: railUpperRight.center,
    second: railDatum.corners.topRight,
    label: "Locate rail upper-right mount",
  });
  const railUpperLeftOnPitch = $.constraint.coincident("railUpperLeftOnPitch", {
    first: railUpperLeft.center,
    second: railDatum.corners.topLeft,
    label: "Locate rail upper-left mount",
  });
  const railFastenerRadius = $.dimension.radius("railFastenerRadius", {
    curve: railLowerLeft.curve,
    value: mm(1.6),
    label: "Rail fastener radius",
    mode: "driving",
  });
  const matchRailLowerRight = $.constraint.equalRadius("matchRailLowerRight", {
    first: railLowerLeft.curve,
    second: railLowerRight.curve,
    label: "Match rail lower-right",
  });
  const matchRailUpperRight = $.constraint.equalRadius("matchRailUpperRight", {
    first: railLowerLeft.curve,
    second: railUpperRight.curve,
    label: "Match rail upper-right",
  });
  const matchRailUpperLeft = $.constraint.equalRadius("matchRailUpperLeft", {
    first: railLowerLeft.curve,
    second: railUpperLeft.curve,
    label: "Match rail upper-left",
  });
  const toolheadPitch = $.operation.rectangle("toolheadPitch", {
    origin: [-12, 0],
    width: mm(24),
    height: mm(1),
    label: "Schematic toolhead mount pitch datum",
    role: "construction",
  });
  const toolheadMountLeft = $.geometry.centerRadiusCircle("toolheadMountLeft", {
    center: [-12, 0],
    radius: mm(2),
    label: "Schematic left toolhead mount",
    role: "profile",
  });
  const toolheadMountRight = $.geometry.centerRadiusCircle("toolheadMountRight", {
    center: [12, 0],
    radius: mm(2),
    label: "Schematic right toolhead mount",
    role: "profile",
  });
  const locateToolheadMountLeft = $.constraint.coincident("locateToolheadMountLeft", {
    first: toolheadMountLeft.center,
    second: toolheadPitch.corners.bottomLeft,
    label: "Locate left toolhead mount",
  });
  const locateToolheadMountRight = $.constraint.coincident("locateToolheadMountRight", {
    first: toolheadMountRight.center,
    second: toolheadPitch.corners.bottomRight,
    label: "Locate right toolhead mount",
  });
  const toolheadMountRadius = $.dimension.radius("toolheadMountRadius", {
    curve: toolheadMountLeft.curve,
    value: mm(2),
    label: "Toolhead mount radius",
    mode: "driving",
  });
  const matchToolheadMount = $.constraint.equalRadius("matchToolheadMount", {
    first: toolheadMountLeft.curve,
    second: toolheadMountRight.curve,
    label: "Matched toolhead mounts",
  });
  $.group("CNC carriage body", [carriageBody, upperWing]);
  $.group("Rail-block datum", [railDatum, railLowerLeft, railLowerRight, railUpperRight, railUpperLeft, railLowerLeftOnPitch, railLowerRightOnPitch, railUpperRightOnPitch, railUpperLeftOnPitch, railFastenerRadius, matchRailLowerRight, matchRailUpperRight, matchRailUpperLeft]);
  $.group("Toolhead interface", [centerline, toolheadPitch, toolheadMountLeft, toolheadMountRight, locateToolheadMountLeft, locateToolheadMountRight, toolheadMountRadius, matchToolheadMount]);
  return {};
});
