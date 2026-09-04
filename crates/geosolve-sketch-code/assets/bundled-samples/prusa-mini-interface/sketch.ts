"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // Front-plane interface study from the published MINI X-carriage model.
  // The 31.423 x 69.407 mm box records only the mesh/solid envelope; the
  // smaller pitch rectangle intentionally diagrams selected M3 interfaces
  // and is not a printable outline or a claim of complete OEM dimensions.
  const carriageEnvelope = $.operation.rectangle("carriageEnvelope", {
    origin: [-15.7115, -34.7035],
    width: mm(31.423),
    height: mm(69.407),
    label: "MINI X-carriage projected envelope",
    role: "construction",
  });
  const bearingSweep = $.operation.slot("bearingSweep", {
    firstCenter: [0, -12],
    secondCenter: [0, 12],
    radius: mm(7.5),
    label: "Schematic linear-bearing clearance datum",
    role: "construction",
  });
  const mountingPitch = $.operation.rectangle("mountingPitch", {
    origin: [-9, -12],
    width: mm(18),
    height: mm(24),
    label: "Schematic selected M3 interface pitch",
    role: "construction",
  });
  const lowerLeftMount = $.geometry.centerRadiusCircle("lowerLeftMount", {
    center: [-9, -12],
    radius: mm(1.6),
    label: "Schematic lower-left M3 interface",
    role: "profile",
  });
  const lowerRightMount = $.geometry.centerRadiusCircle("lowerRightMount", {
    center: [9, -12],
    radius: mm(1.6),
    label: "Schematic lower-right M3 interface",
    role: "profile",
  });
  const upperRightMount = $.geometry.centerRadiusCircle("upperRightMount", {
    center: [9, 12],
    radius: mm(1.6),
    label: "Schematic upper-right M3 interface",
    role: "profile",
  });
  const upperLeftMount = $.geometry.centerRadiusCircle("upperLeftMount", {
    center: [-9, 12],
    radius: mm(1.6),
    label: "Schematic upper-left M3 interface",
    role: "profile",
  });
  const lowerLeftOnPitch = $.constraint.coincident("lowerLeftOnPitch", {
    first: lowerLeftMount.center,
    second: mountingPitch.corners.bottomLeft,
    label: "Locate lower-left mount on pitch datum",
  });
  const lowerRightOnPitch = $.constraint.coincident("lowerRightOnPitch", {
    first: lowerRightMount.center,
    second: mountingPitch.corners.bottomRight,
    label: "Locate lower-right mount on pitch datum",
  });
  const upperRightOnPitch = $.constraint.coincident("upperRightOnPitch", {
    first: upperRightMount.center,
    second: mountingPitch.corners.topRight,
    label: "Locate upper-right mount on pitch datum",
  });
  const upperLeftOnPitch = $.constraint.coincident("upperLeftOnPitch", {
    first: upperLeftMount.center,
    second: mountingPitch.corners.topLeft,
    label: "Locate upper-left mount on pitch datum",
  });
  const mountRadius = $.dimension.radius("mountRadius", {
    curve: lowerLeftMount.curve,
    value: mm(1.6),
    label: "M3 interface radius",
    mode: "driving",
  });
  const matchLowerRight = $.constraint.equalRadius("matchLowerRight", {
    first: lowerLeftMount.curve,
    second: lowerRightMount.curve,
    label: "Match lower-right mount",
  });
  const matchUpperRight = $.constraint.equalRadius("matchUpperRight", {
    first: lowerLeftMount.curve,
    second: upperRightMount.curve,
    label: "Match upper-right mount",
  });
  const matchUpperLeft = $.constraint.equalRadius("matchUpperLeft", {
    first: lowerLeftMount.curve,
    second: upperLeftMount.curve,
    label: "Match upper-left mount",
  });
  $.group("Published X-carriage envelope", [carriageEnvelope]);
  $.group("Bearing and belt datum", [bearingSweep]);
  $.group("Selected mounting interface", [mountingPitch, lowerLeftMount, lowerRightMount, upperRightMount, upperLeftMount, lowerLeftOnPitch, lowerRightOnPitch, upperRightOnPitch, upperLeftOnPitch, mountRadius, matchLowerRight, matchUpperRight, matchUpperLeft]);
  return {};
});
