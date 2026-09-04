"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // Front-plane study only. The 64.1 x 32.4 mm outline is the published LH
  // lower-body model envelope; the belt and MGN9 features are selected datum
  // diagrams, not an imported projection or a replacement carriage part.
  const adapterPlate = $.operation.rectangle("adapterPlate", {
    origin: [-32.05, -16.2],
    width: mm(64.1),
    height: mm(32.4),
    label: "HD9 LH lower-body front envelope",
    role: "profile",
  });
  const beltPassage = $.operation.slot("beltPassage", {
    firstCenter: [-17, 10],
    secondCenter: [17, 10],
    radius: mm(2.5),
    label: "Schematic HD9 belt-line clearance",
    role: "profile",
  });
  const railEnvelope = $.operation.rectangle("railEnvelope", {
    origin: [-4.5, -45],
    width: mm(9),
    height: mm(90),
    label: "MGN9 rail envelope",
    role: "construction",
  });
  const carriageEnvelope = $.operation.rectangle("carriageEnvelope", {
    origin: [-10, -20],
    width: mm(20),
    height: mm(40),
    label: "MGN9H carriage envelope",
    role: "construction",
  });
  const mountingPitch = $.operation.rectangle("mountingPitch", {
    origin: [-7.5, -10],
    width: mm(15),
    height: mm(20),
    label: "MGN9 mounting-pitch datum",
    role: "construction",
  });
  const mountLowerLeft = $.geometry.centerRadiusCircle("mountLowerLeft", {
    center: [-7.5, -10],
    radius: mm(1.6),
    label: "Lower-left M3 clearance",
    role: "profile",
  });
  const mountLowerRight = $.geometry.centerRadiusCircle("mountLowerRight", {
    center: [7.5, -10],
    radius: mm(1.6),
    label: "Lower-right M3 clearance",
    role: "profile",
  });
  const mountUpperRight = $.geometry.centerRadiusCircle("mountUpperRight", {
    center: [7.5, 10],
    radius: mm(1.6),
    label: "Upper-right M3 clearance",
    role: "profile",
  });
  const mountUpperLeft = $.geometry.centerRadiusCircle("mountUpperLeft", {
    center: [-7.5, 10],
    radius: mm(1.6),
    label: "Upper-left M3 clearance",
    role: "profile",
  });
  const mountLowerLeftOnPitch = $.constraint.coincident("mountLowerLeftOnPitch", {
    first: mountLowerLeft.center,
    second: mountingPitch.corners.bottomLeft,
    label: "Locate lower-left mount on pitch datum",
  });
  const mountLowerRightOnPitch = $.constraint.coincident("mountLowerRightOnPitch", {
    first: mountLowerRight.center,
    second: mountingPitch.corners.bottomRight,
    label: "Locate lower-right mount on pitch datum",
  });
  const mountUpperRightOnPitch = $.constraint.coincident("mountUpperRightOnPitch", {
    first: mountUpperRight.center,
    second: mountingPitch.corners.topRight,
    label: "Locate upper-right mount on pitch datum",
  });
  const mountUpperLeftOnPitch = $.constraint.coincident("mountUpperLeftOnPitch", {
    first: mountUpperLeft.center,
    second: mountingPitch.corners.topLeft,
    label: "Locate upper-left mount on pitch datum",
  });
  const mountRadius = $.dimension.radius("mountRadius", {
    curve: mountLowerLeft.curve,
    value: mm(1.6),
    label: "M3 clearance radius",
    mode: "driving",
  });
  const matchLowerRight = $.constraint.equalRadius("matchLowerRight", {
    first: mountLowerLeft.curve,
    second: mountLowerRight.curve,
    label: "Match lower-right clearance",
  });
  const matchUpperRight = $.constraint.equalRadius("matchUpperRight", {
    first: mountLowerLeft.curve,
    second: mountUpperRight.curve,
    label: "Match upper-right clearance",
  });
  const matchUpperLeft = $.constraint.equalRadius("matchUpperLeft", {
    first: mountLowerLeft.curve,
    second: mountUpperLeft.curve,
    label: "Match upper-left clearance",
  });
  $.group("HD9 carriage envelope", [adapterPlate, beltPassage]);
  $.group("MGN9 rail datum", [railEnvelope, carriageEnvelope, mountingPitch]);
  $.group("MGN9 mounting interface", [mountLowerLeft, mountLowerRight, mountUpperRight, mountUpperLeft, mountLowerLeftOnPitch, mountLowerRightOnPitch, mountUpperRightOnPitch, mountUpperLeftOnPitch, mountRadius, matchLowerRight, matchUpperRight, matchUpperLeft]);
  return {};
});
