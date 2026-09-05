"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // Two adjacent datum studies, not one interchangeable mounting pattern.
  // The HD9 STEP normal-to-Y R1.6 axes at X=5/25 and Z=150/170
  // form a 20 x 20 mm square. That pattern is centred in the left study.
  // The right nominal MGN9H study uses a 15 x 16 mm fastener pitch.
  // The 64.1 x 32.4 envelope uses the same source origin (X-15,Z-160).
  const adapterPlate = $.operation.rectangle("adapterPlate", {
    origin: [-50, -16.2],
    width: mm(64.1),
    height: mm(32.4),
    label: "Published HD9 XZ envelope relative to mount centre",
    role: "construction",
  });
  const hd9Pitch = $.geometry.centerRectangle("hd9Pitch", {
    center: [0, 0],
    corner: [10, 10],
    label: "Extracted HD9 20 x 20 pattern",
    role: "construction",
  });
  const hd9PitchOrigin = $.constraint.fixedPoint("hd9PitchOrigin", {
    point: hd9Pitch.center,
    target: [0, 0],
    label: "Extracted HD9 20 x 20 pattern centre datum",
  });
  const hd9PitchWidth = $.dimension.curveLength("hd9PitchWidth", {
    curve: hd9Pitch.spans[0],
    value: mm(20),
    mode: "driving",
    label: "Extracted HD9 20 x 20 pattern width",
  });
  const hd9PitchHeight = $.dimension.curveLength("hd9PitchHeight", {
    curve: hd9Pitch.spans[1],
    value: mm(20),
    mode: "driving",
    label: "Extracted HD9 20 x 20 pattern height",
  });
  const hd9PitchHole1 = $.geometry.centerRadiusCircle("hd9PitchHole1", {
    center: hd9Pitch.corners[0],
    radius: mm(1.6),
    label: "Extracted HD9 20 x 20 pattern hole 1",
    role: "profile",
  });
  const hd9PitchRadius = $.dimension.radius("hd9PitchRadius", {
    curve: hd9PitchHole1.curve,
    value: mm(1.6),
    mode: "driving",
    label: "Extracted HD9 20 x 20 pattern clearance radius",
  });
  const hd9PitchHole2 = $.geometry.centerRadiusCircle("hd9PitchHole2", {
    center: hd9Pitch.corners[1],
    radius: mm(1.6),
    label: "Extracted HD9 20 x 20 pattern hole 2",
    role: "profile",
  });
  const hd9PitchHole2Match = $.constraint.equalRadius("hd9PitchHole2Match", {
    first: hd9PitchHole1.curve,
    second: hd9PitchHole2.curve,
    label: "Extracted HD9 20 x 20 pattern equal clearances",
  });
  const hd9PitchHole3 = $.geometry.centerRadiusCircle("hd9PitchHole3", {
    center: hd9Pitch.corners[2],
    radius: mm(1.6),
    label: "Extracted HD9 20 x 20 pattern hole 3",
    role: "profile",
  });
  const hd9PitchHole3Match = $.constraint.equalRadius("hd9PitchHole3Match", {
    first: hd9PitchHole1.curve,
    second: hd9PitchHole3.curve,
    label: "Extracted HD9 20 x 20 pattern equal clearances",
  });
  const hd9PitchHole4 = $.geometry.centerRadiusCircle("hd9PitchHole4", {
    center: hd9Pitch.corners[3],
    radius: mm(1.6),
    label: "Extracted HD9 20 x 20 pattern hole 4",
    role: "profile",
  });
  const hd9PitchHole4Match = $.constraint.equalRadius("hd9PitchHole4Match", {
    first: hd9PitchHole1.curve,
    second: hd9PitchHole4.curve,
    label: "Extracted HD9 20 x 20 pattern equal clearances",
  });
  const railEnvelope = $.operation.rectangle("railEnvelope", {
    origin: [53.5, -30],
    width: mm(9),
    height: mm(60),
    label: "Nominal MGN9 rail, illustrative length",
    role: "construction",
  });
  const carriageEnvelope = $.operation.rectangle("carriageEnvelope", {
    origin: [48, -19.95],
    width: mm(20),
    height: mm(39.9),
    label: "Nominal MGN9H block envelope",
    role: "construction",
  });
  const mountingPitch = $.geometry.centerRectangle("mountingPitch", {
    center: [58, 0],
    corner: [65.5, 8],
    label: "Nominal MGN9H 15 x 16 mounting pitch",
    role: "construction",
  });
  const mountingPitchOrigin = $.constraint.fixedPoint("mountingPitchOrigin", {
    point: mountingPitch.center,
    target: [58, 0],
    label: "Nominal MGN9H 15 x 16 mounting pitch centre datum",
  });
  const mountingPitchWidth = $.dimension.curveLength("mountingPitchWidth", {
    curve: mountingPitch.spans[0],
    value: mm(15),
    mode: "driving",
    label: "Nominal MGN9H 15 x 16 mounting pitch width",
  });
  const mountingPitchHeight = $.dimension.curveLength("mountingPitchHeight", {
    curve: mountingPitch.spans[1],
    value: mm(16),
    mode: "driving",
    label: "Nominal MGN9H 15 x 16 mounting pitch height",
  });
  const mountingPitchHole1 = $.geometry.centerRadiusCircle("mountingPitchHole1", {
    center: mountingPitch.corners[0],
    radius: mm(1.6),
    label: "Nominal MGN9H 15 x 16 mounting pitch hole 1",
    role: "profile",
  });
  const mountingPitchRadius = $.dimension.radius("mountingPitchRadius", {
    curve: mountingPitchHole1.curve,
    value: mm(1.6),
    mode: "driving",
    label: "Nominal MGN9H 15 x 16 mounting pitch clearance radius",
  });
  const mountingPitchHole2 = $.geometry.centerRadiusCircle("mountingPitchHole2", {
    center: mountingPitch.corners[1],
    radius: mm(1.6),
    label: "Nominal MGN9H 15 x 16 mounting pitch hole 2",
    role: "profile",
  });
  const mountingPitchHole2Match = $.constraint.equalRadius("mountingPitchHole2Match", {
    first: mountingPitchHole1.curve,
    second: mountingPitchHole2.curve,
    label: "Nominal MGN9H 15 x 16 mounting pitch equal clearances",
  });
  const mountingPitchHole3 = $.geometry.centerRadiusCircle("mountingPitchHole3", {
    center: mountingPitch.corners[2],
    radius: mm(1.6),
    label: "Nominal MGN9H 15 x 16 mounting pitch hole 3",
    role: "profile",
  });
  const mountingPitchHole3Match = $.constraint.equalRadius("mountingPitchHole3Match", {
    first: mountingPitchHole1.curve,
    second: mountingPitchHole3.curve,
    label: "Nominal MGN9H 15 x 16 mounting pitch equal clearances",
  });
  const mountingPitchHole4 = $.geometry.centerRadiusCircle("mountingPitchHole4", {
    center: mountingPitch.corners[3],
    radius: mm(1.6),
    label: "Nominal MGN9H 15 x 16 mounting pitch hole 4",
    role: "profile",
  });
  const mountingPitchHole4Match = $.constraint.equalRadius("mountingPitchHole4Match", {
    first: mountingPitchHole1.curve,
    second: mountingPitchHole4.curve,
    label: "Nominal MGN9H 15 x 16 mounting pitch equal clearances",
  });
  $.group("HD9 carriage envelope", [adapterPlate, hd9Pitch, hd9PitchOrigin, hd9PitchWidth, hd9PitchHeight, hd9PitchHole1, hd9PitchRadius, hd9PitchHole2, hd9PitchHole2Match, hd9PitchHole3, hd9PitchHole3Match, hd9PitchHole4, hd9PitchHole4Match]);
  $.group("MGN9 rail datum", [railEnvelope, carriageEnvelope]);
  $.group("MGN9 mounting interface", [mountingPitch, mountingPitchOrigin, mountingPitchWidth, mountingPitchHeight, mountingPitchHole1, mountingPitchRadius, mountingPitchHole2, mountingPitchHole2Match, mountingPitchHole3, mountingPitchHole3Match, mountingPitchHole4, mountingPitchHole4Match]);
  return {};
});
