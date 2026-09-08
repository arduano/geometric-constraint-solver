"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
import { harnessRoute } from "./patches/harness-route.patch.ts";

export default sketch({
  title: "Dense robotic harness-routing backplane",
  description: "Eight readable harness trunks generate keyed clips and computed bend fillets across a complete robotic cell backplane.",
}, ($) => {
  // A 360 × 220 mm backplane carries eight source-authored harness trunks.
  // The patch maps every keyed vertex to a clip and every keyed corner to a
  // computed Fillet, yielding 80 clips and 64 branch-explicit bend features.
  const backplane = $.geometry.twoPointAlignedRectangle("backplane", {
    firstCorner: [-180, -110],
    oppositeCorner: [180, 110],
    role: "profile",
    label: "Robotic cell backplane",
  });
  const lowerWestMount = $.geometry.centerRadiusCircle("lowerWestMount", {
    center: [-166, -104],
    radius: mm(4),
    role: "profile",
    label: "Lower-west M8 clearance",
  });
  const lowerInnerWestMount = $.geometry.centerRadiusCircle("lowerInnerWestMount", {
    center: [-55.33333333333333, -104],
    radius: mm(4),
    role: "profile",
    label: "Lower inner-west M8 clearance",
  });
  const lowerInnerEastMount = $.geometry.centerRadiusCircle("lowerInnerEastMount", {
    center: [55.33333333333334, -104],
    radius: mm(4),
    role: "profile",
    label: "Lower inner-east M8 clearance",
  });
  const lowerEastMount = $.geometry.centerRadiusCircle("lowerEastMount", {
    center: [166, -104],
    radius: mm(4),
    role: "profile",
    label: "Lower-east M8 clearance",
  });
  const upperWestMount = $.geometry.centerRadiusCircle("upperWestMount", {
    center: [-166, 96],
    radius: mm(4),
    role: "profile",
    label: "Upper-west M8 clearance",
  });
  const upperInnerWestMount = $.geometry.centerRadiusCircle("upperInnerWestMount", {
    center: [-55.33333333333333, 96],
    radius: mm(4),
    role: "profile",
    label: "Upper inner-west M8 clearance",
  });
  const upperInnerEastMount = $.geometry.centerRadiusCircle("upperInnerEastMount", {
    center: [55.33333333333334, 96],
    radius: mm(4),
    role: "profile",
    label: "Upper inner-east M8 clearance",
  });
  const upperEastMount = $.geometry.centerRadiusCircle("upperEastMount", {
    center: [166, 96],
    radius: mm(4),
    role: "profile",
    label: "Upper-east M8 clearance",
  });
  // Lower mounts sit below the service trunk so mounting bores and clip holes
  // remain distinct in the complete 2D layout.
  // The route geometry is intentionally repetitive in structure but not in
  // placement. Each keyed bend remains readable and independently selectable.
  const powerBus = $.geometry.polyline("powerBus", {
    vertices: [{
      key: "source",
      position: [-156, 82],
    }, {
      key: "entry",
      position: [-136, 82],
    }, {
      key: "strainReliefA",
      position: [-120, 70],
    }, {
      key: "leftGuide",
      position: [-94, 70],
    }, {
      key: "leftTurn",
      position: [-74, 86],
    }, {
      key: "midspan",
      position: [-36, 86],
    }, {
      key: "rightTurn",
      position: [8, 74],
    }, {
      key: "rightGuide",
      position: [48, 74],
    }, {
      key: "strainReliefB",
      position: [116, 82],
    }, {
      key: "sink",
      position: [156, 82],
    }],
    closed: false,
    branchDirections: [[1, 0], [0.8, -0.6], [1, 0], [0.7808688094430304, 0.6246950475544243], [1, 0], [0.9647638212377322, -0.2631174057921088], [1, 0], [0.9938837346736188, 0.11043152607484653], [1, 0]],
    role: "profile",
    label: "High-current power bus",
  });
  const servoABus = $.geometry.polyline("servoABus", {
    vertices: [{
      key: "source",
      position: [-156, 58],
    }, {
      key: "entry",
      position: [-132, 58],
    }, {
      key: "strainReliefA",
      position: [-116, 46],
    }, {
      key: "leftGuide",
      position: [-86, 46],
    }, {
      key: "leftTurn",
      position: [-66, 62],
    }, {
      key: "midspan",
      position: [-24, 62],
    }, {
      key: "rightTurn",
      position: [16, 50],
    }, {
      key: "rightGuide",
      position: [58, 50],
    }, {
      key: "strainReliefB",
      position: [120, 58],
    }, {
      key: "sink",
      position: [156, 58],
    }],
    closed: false,
    role: "profile",
    label: "Servo A trunk",
  });
  const servoBBus = $.geometry.polyline("servoBBus", {
    vertices: [{
      key: "source",
      position: [-156, 34],
    }, {
      key: "entry",
      position: [-134, 34],
    }, {
      key: "strainReliefA",
      position: [-118, 22],
    }, {
      key: "leftGuide",
      position: [-88, 22],
    }, {
      key: "leftTurn",
      position: [-68, 38],
    }, {
      key: "midspan",
      position: [-30, 38],
    }, {
      key: "rightTurn",
      position: [12, 26],
    }, {
      key: "rightGuide",
      position: [54, 26],
    }, {
      key: "strainReliefB",
      position: [118, 34],
    }, {
      key: "sink",
      position: [156, 34],
    }],
    closed: false,
    role: "profile",
    label: "Servo B trunk",
  });
  const sensorABus = $.geometry.polyline("sensorABus", {
    vertices: [{
      key: "source",
      position: [-156, 10],
    }, {
      key: "entry",
      position: [-130, 10],
    }, {
      key: "strainReliefA",
      position: [-112, -2],
    }, {
      key: "leftGuide",
      position: [-82, -2],
    }, {
      key: "leftTurn",
      position: [-62, 14],
    }, {
      key: "midspan",
      position: [-22, 14],
    }, {
      key: "rightTurn",
      position: [20, 2],
    }, {
      key: "rightGuide",
      position: [60, 2],
    }, {
      key: "strainReliefB",
      position: [122, 10],
    }, {
      key: "sink",
      position: [156, 10],
    }],
    closed: false,
    role: "profile",
    label: "Sensor A trunk",
  });
  const sensorBBus = $.geometry.polyline("sensorBBus", {
    vertices: [{
      key: "source",
      position: [-156, -14],
    }, {
      key: "entry",
      position: [-132, -14],
    }, {
      key: "strainReliefA",
      position: [-114, -26],
    }, {
      key: "leftGuide",
      position: [-84, -26],
    }, {
      key: "leftTurn",
      position: [-64, -10],
    }, {
      key: "midspan",
      position: [-24, -10],
    }, {
      key: "rightTurn",
      position: [18, -22],
    }, {
      key: "rightGuide",
      position: [58, -22],
    }, {
      key: "strainReliefB",
      position: [120, -14],
    }, {
      key: "sink",
      position: [156, -14],
    }],
    closed: false,
    role: "profile",
    label: "Sensor B trunk",
  });
  const gripperBus = $.geometry.polyline("gripperBus", {
    vertices: [{
      key: "source",
      position: [-156, -38],
    }, {
      key: "entry",
      position: [-134, -38],
    }, {
      key: "strainReliefA",
      position: [-116, -50],
    }, {
      key: "leftGuide",
      position: [-86, -50],
    }, {
      key: "leftTurn",
      position: [-66, -34],
    }, {
      key: "midspan",
      position: [-28, -34],
    }, {
      key: "rightTurn",
      position: [14, -46],
    }, {
      key: "rightGuide",
      position: [56, -46],
    }, {
      key: "strainReliefB",
      position: [118, -38],
    }, {
      key: "sink",
      position: [156, -38],
    }],
    closed: false,
    role: "profile",
    label: "Gripper trunk",
  });
  const visionBus = $.geometry.polyline("visionBus", {
    vertices: [{
      key: "source",
      position: [-156, -62],
    }, {
      key: "entry",
      position: [-132, -62],
    }, {
      key: "strainReliefA",
      position: [-114, -74],
    }, {
      key: "leftGuide",
      position: [-84, -74],
    }, {
      key: "leftTurn",
      position: [-64, -58],
    }, {
      key: "midspan",
      position: [-24, -58],
    }, {
      key: "rightTurn",
      position: [18, -70],
    }, {
      key: "rightGuide",
      position: [58, -70],
    }, {
      key: "strainReliefB",
      position: [120, -62],
    }, {
      key: "sink",
      position: [156, -62],
    }],
    closed: false,
    role: "profile",
    label: "Machine-vision trunk",
  });
  const serviceBus = $.geometry.polyline("serviceBus", {
    vertices: [{
      key: "source",
      position: [-156, -86],
    }, {
      key: "entry",
      position: [-136, -86],
    }, {
      key: "strainReliefA",
      position: [-118, -98],
    }, {
      key: "leftGuide",
      position: [-90, -98],
    }, {
      key: "leftTurn",
      position: [-70, -82],
    }, {
      key: "serviceLoop",
      position: [-30, -82],
    }, {
      key: "rightTurn",
      position: [12, -94],
    }, {
      key: "rightGuide",
      position: [54, -94],
    }, {
      key: "strainReliefB",
      position: [118, -86],
    }, {
      key: "sink",
      position: [156, -86],
    }],
    closed: false,
    role: "profile",
    label: "Service-loop trunk",
  });
  const clipRadius = $.parameter("clipRadius", mm(2.4), {
    label: "Clip radius",
    description: "Shared clip radius.",
    isKeyParameter: true,
  });
  const bendRadius = $.parameter("bendRadius", mm(5), {
    label: "Bend radius",
    description: "Shared centreline bend radius.",
    isKeyParameter: true,
  });
  const powerHarness = $.use("powerHarness", harnessRoute, {
    vertices: powerBus.vertices,
    corners: powerBus.filletableCorners,
    clipRadius: clipRadius,
    bendRadius: bendRadius,
  });
  const servoAHarness = $.use("servoAHarness", harnessRoute, {
    vertices: servoABus.vertices,
    corners: servoABus.filletableCorners,
    clipRadius: clipRadius,
    bendRadius: bendRadius,
  });
  const servoBHarness = $.use("servoBHarness", harnessRoute, {
    vertices: servoBBus.vertices,
    corners: servoBBus.filletableCorners,
    clipRadius: clipRadius,
    bendRadius: bendRadius,
  });
  const sensorAHarness = $.use("sensorAHarness", harnessRoute, {
    vertices: sensorABus.vertices,
    corners: sensorABus.filletableCorners,
    clipRadius: clipRadius,
    bendRadius: bendRadius,
  });
  const sensorBHarness = $.use("sensorBHarness", harnessRoute, {
    vertices: sensorBBus.vertices,
    corners: sensorBBus.filletableCorners,
    clipRadius: clipRadius,
    bendRadius: bendRadius,
  });
  const gripperHarness = $.use("gripperHarness", harnessRoute, {
    vertices: gripperBus.vertices,
    corners: gripperBus.filletableCorners,
    clipRadius: clipRadius,
    bendRadius: bendRadius,
  });
  const visionHarness = $.use("visionHarness", harnessRoute, {
    vertices: visionBus.vertices,
    corners: visionBus.filletableCorners,
    clipRadius: clipRadius,
    bendRadius: bendRadius,
  });
  const serviceHarness = $.use("serviceHarness", harnessRoute, {
    vertices: serviceBus.vertices,
    corners: serviceBus.filletableCorners,
    clipRadius: clipRadius,
    bendRadius: bendRadius,
  });
  $.group("Backplane and mounting grid", [backplane, lowerWestMount, lowerInnerWestMount, lowerInnerEastMount, lowerEastMount, upperWestMount, upperInnerWestMount, upperInnerEastMount, upperEastMount]);
  $.group("Power and servo harnesses", [powerBus, powerHarness, servoABus, servoAHarness, servoBBus, servoBHarness]);
  $.group("Sensor harnesses", [sensorABus, sensorAHarness, sensorBBus, sensorBHarness]);
  $.group("Tooling vision and service harnesses", [gripperBus, gripperHarness, visionBus, visionHarness, serviceBus, serviceHarness]);
  return {
    board: backplane,
    mounting: {
      lowerWest: lowerWestMount,
      lowerInnerWest: lowerInnerWestMount,
      lowerInnerEast: lowerInnerEastMount,
      lowerEast: lowerEastMount,
      upperWest: upperWestMount,
      upperInnerWest: upperInnerWestMount,
      upperInnerEast: upperInnerEastMount,
      upperEast: upperEastMount,
    },
    harnesses: {
      power: powerHarness,
      servoA: servoAHarness,
      servoB: servoBHarness,
      sensorA: sensorAHarness,
      sensorB: sensorBHarness,
      gripper: gripperHarness,
      vision: visionHarness,
      service: serviceHarness,
    },
  };
});
