// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { harnessRoute } from "./patches/harness-route.patch.ts";

export default sketch(($) => {
  // A 360 x 220 mm perforated fixture board carries two connector banks and
  // eight independently keyed harness routes. Native geometry and constraints
  // remain authoritative; the reusable patch owns only adaptive clips/Fillets.
  const board = $.geometry.rectangle("board", {
    lowerLeft: [-180, -110],
    upperRight: [180, 110],
  });
  const boardAnchor = $.constraint.fixedPoint("boardAnchor", {
    point: board.corners.lowerLeft,
    target: [-180, -110],
  });
  const boardWidth = $.dimension.curveLength("boardWidth", {
    curve: board.edges.bottom,
    target: mm(360),
  });
  const boardHeight = $.dimension.curveLength("boardHeight", {
    curve: board.edges.right,
    target: mm(220),
  });

  const boreNw = $.geometry.circle("boreNw", { center: [-166, 96], radius: mm(4) });
  const boreNe = $.geometry.circle("boreNe", { center: [166, 96], radius: mm(4) });
  const boreSe = $.geometry.circle("boreSe", { center: [166, -96], radius: mm(4) });
  const boreSw = $.geometry.circle("boreSw", { center: [-166, -96], radius: mm(4) });
  const boreNwFixed = $.constraint.fixedPoint("boreNwFixed", { point: boreNw.center, target: [-166, 96] });
  const boreNeFixed = $.constraint.fixedPoint("boreNeFixed", { point: boreNe.center, target: [166, 96] });
  const boreSeFixed = $.constraint.fixedPoint("boreSeFixed", { point: boreSe.center, target: [166, -96] });
  const boreSwFixed = $.constraint.fixedPoint("boreSwFixed", { point: boreSw.center, target: [-166, -96] });

  // Explicit connector centres make the board manufacturable-style source,
  // while leaving generated clip centres derived from route vertices.
  const leftPower = $.geometry.circle("leftPower", { center: [-156, 82], radius: mm(6) });
  const rightPower = $.geometry.circle("rightPower", { center: [156, 82], radius: mm(6) });
  const leftServoA = $.geometry.circle("leftServoA", { center: [-156, 58], radius: mm(6) });
  const rightServoA = $.geometry.circle("rightServoA", { center: [156, 58], radius: mm(6) });
  const leftServoB = $.geometry.circle("leftServoB", { center: [-156, 34], radius: mm(6) });
  const rightServoB = $.geometry.circle("rightServoB", { center: [156, 34], radius: mm(6) });
  const leftSensorA = $.geometry.circle("leftSensorA", { center: [-156, 10], radius: mm(6) });
  const rightSensorA = $.geometry.circle("rightSensorA", { center: [156, 10], radius: mm(6) });
  const leftSensorB = $.geometry.circle("leftSensorB", { center: [-156, -14], radius: mm(6) });
  const rightSensorB = $.geometry.circle("rightSensorB", { center: [156, -14], radius: mm(6) });
  const leftGripper = $.geometry.circle("leftGripper", { center: [-156, -38], radius: mm(6) });
  const rightGripper = $.geometry.circle("rightGripper", { center: [156, -38], radius: mm(6) });
  const leftVision = $.geometry.circle("leftVision", { center: [-156, -62], radius: mm(6) });
  const rightVision = $.geometry.circle("rightVision", { center: [156, -62], radius: mm(6) });
  const leftService = $.geometry.circle("leftService", { center: [-156, -86], radius: mm(6) });
  const rightService = $.geometry.circle("rightService", { center: [156, -86], radius: mm(6) });
  const leftPowerFixed = $.constraint.fixedPoint("leftPowerFixed", { point: leftPower.center, target: [-156, 82] });
  const rightPowerFixed = $.constraint.fixedPoint("rightPowerFixed", { point: rightPower.center, target: [156, 82] });
  const leftServoAFixed = $.constraint.fixedPoint("leftServoAFixed", { point: leftServoA.center, target: [-156, 58] });
  const rightServoAFixed = $.constraint.fixedPoint("rightServoAFixed", { point: rightServoA.center, target: [156, 58] });
  const leftServoBFixed = $.constraint.fixedPoint("leftServoBFixed", { point: leftServoB.center, target: [-156, 34] });
  const rightServoBFixed = $.constraint.fixedPoint("rightServoBFixed", { point: rightServoB.center, target: [156, 34] });
  const leftSensorAFixed = $.constraint.fixedPoint("leftSensorAFixed", { point: leftSensorA.center, target: [-156, 10] });
  const rightSensorAFixed = $.constraint.fixedPoint("rightSensorAFixed", { point: rightSensorA.center, target: [156, 10] });
  const leftSensorBFixed = $.constraint.fixedPoint("leftSensorBFixed", { point: leftSensorB.center, target: [-156, -14] });
  const rightSensorBFixed = $.constraint.fixedPoint("rightSensorBFixed", { point: rightSensorB.center, target: [156, -14] });
  const leftGripperFixed = $.constraint.fixedPoint("leftGripperFixed", { point: leftGripper.center, target: [-156, -38] });
  const rightGripperFixed = $.constraint.fixedPoint("rightGripperFixed", { point: rightGripper.center, target: [156, -38] });
  const leftVisionFixed = $.constraint.fixedPoint("leftVisionFixed", { point: leftVision.center, target: [-156, -62] });
  const rightVisionFixed = $.constraint.fixedPoint("rightVisionFixed", { point: rightVision.center, target: [156, -62] });
  const leftServiceFixed = $.constraint.fixedPoint("leftServiceFixed", { point: leftService.center, target: [-156, -86] });
  const rightServiceFixed = $.constraint.fixedPoint("rightServiceFixed", { point: rightService.center, target: [156, -86] });

  const powerRoute = $.geometry.polyline("powerRoute", {
    vertices: [
      { key: "source", position: [-156, 82] },
      { key: "entry", position: [-136, 82] },
      { key: "strainReliefA", position: [-120, 70] },
      { key: "leftGuide", position: [-94, 70] },
      { key: "leftTurn", position: [-74, 86] },
      { key: "midspan", position: [-36, 86] },
      { key: "rightTurn", position: [8, 74] },
      { key: "rightGuide", position: [48, 74] },
      { key: "strainReliefB", position: [116, 82] },
      { key: "sink", position: [156, 82] },
    ],
    closed: false,
  });
  const powerSourceFixed = $.constraint.fixedPoint("powerSourceFixed", { point: powerRoute.vertices.byKey.source, target: [-156, 82] });
  const powerSinkFixed = $.constraint.fixedPoint("powerSinkFixed", { point: powerRoute.vertices.byKey.sink, target: [156, 82] });

  const servoARoute = $.geometry.polyline("servoARoute", {
    vertices: [
      { key: "source", position: [-156, 58] },
      { key: "entry", position: [-132, 58] },
      { key: "strainReliefA", position: [-116, 46] },
      { key: "leftGuide", position: [-86, 46] },
      { key: "leftTurn", position: [-66, 62] },
      { key: "midspan", position: [-24, 62] },
      { key: "rightTurn", position: [16, 50] },
      { key: "rightGuide", position: [58, 50] },
      { key: "strainReliefB", position: [120, 58] },
      { key: "sink", position: [156, 58] },
    ],
    closed: false,
  });
  const servoASourceFixed = $.constraint.fixedPoint("servoASourceFixed", { point: servoARoute.vertices.byKey.source, target: [-156, 58] });
  const servoASinkFixed = $.constraint.fixedPoint("servoASinkFixed", { point: servoARoute.vertices.byKey.sink, target: [156, 58] });

  const servoBRoute = $.geometry.polyline("servoBRoute", {
    vertices: [
      { key: "source", position: [-156, 34] },
      { key: "entry", position: [-134, 34] },
      { key: "strainReliefA", position: [-118, 22] },
      { key: "leftGuide", position: [-88, 22] },
      { key: "leftTurn", position: [-68, 38] },
      { key: "midspan", position: [-30, 38] },
      { key: "rightTurn", position: [12, 26] },
      { key: "rightGuide", position: [54, 26] },
      { key: "strainReliefB", position: [118, 34] },
      { key: "sink", position: [156, 34] },
    ],
    closed: false,
  });
  const servoBSourceFixed = $.constraint.fixedPoint("servoBSourceFixed", { point: servoBRoute.vertices.byKey.source, target: [-156, 34] });
  const servoBSinkFixed = $.constraint.fixedPoint("servoBSinkFixed", { point: servoBRoute.vertices.byKey.sink, target: [156, 34] });

  const sensorARoute = $.geometry.polyline("sensorARoute", {
    vertices: [
      { key: "source", position: [-156, 10] },
      { key: "entry", position: [-130, 10] },
      { key: "strainReliefA", position: [-112, -2] },
      { key: "leftGuide", position: [-82, -2] },
      { key: "leftTurn", position: [-62, 14] },
      { key: "midspan", position: [-22, 14] },
      { key: "rightTurn", position: [20, 2] },
      { key: "rightGuide", position: [60, 2] },
      { key: "strainReliefB", position: [122, 10] },
      { key: "sink", position: [156, 10] },
    ],
    closed: false,
  });
  const sensorASourceFixed = $.constraint.fixedPoint("sensorASourceFixed", { point: sensorARoute.vertices.byKey.source, target: [-156, 10] });
  const sensorASinkFixed = $.constraint.fixedPoint("sensorASinkFixed", { point: sensorARoute.vertices.byKey.sink, target: [156, 10] });

  const sensorBRoute = $.geometry.polyline("sensorBRoute", {
    vertices: [
      { key: "source", position: [-156, -14] },
      { key: "entry", position: [-132, -14] },
      { key: "strainReliefA", position: [-114, -26] },
      { key: "leftGuide", position: [-84, -26] },
      { key: "leftTurn", position: [-64, -10] },
      { key: "midspan", position: [-24, -10] },
      { key: "rightTurn", position: [18, -22] },
      { key: "rightGuide", position: [58, -22] },
      { key: "strainReliefB", position: [120, -14] },
      { key: "sink", position: [156, -14] },
    ],
    closed: false,
  });
  const sensorBSourceFixed = $.constraint.fixedPoint("sensorBSourceFixed", { point: sensorBRoute.vertices.byKey.source, target: [-156, -14] });
  const sensorBSinkFixed = $.constraint.fixedPoint("sensorBSinkFixed", { point: sensorBRoute.vertices.byKey.sink, target: [156, -14] });

  const gripperRoute = $.geometry.polyline("gripperRoute", {
    vertices: [
      { key: "source", position: [-156, -38] },
      { key: "entry", position: [-134, -38] },
      { key: "strainReliefA", position: [-116, -50] },
      { key: "leftGuide", position: [-86, -50] },
      { key: "leftTurn", position: [-66, -34] },
      { key: "midspan", position: [-28, -34] },
      { key: "rightTurn", position: [14, -46] },
      { key: "rightGuide", position: [56, -46] },
      { key: "strainReliefB", position: [118, -38] },
      { key: "sink", position: [156, -38] },
    ],
    closed: false,
  });
  const gripperSourceFixed = $.constraint.fixedPoint("gripperSourceFixed", { point: gripperRoute.vertices.byKey.source, target: [-156, -38] });
  const gripperSinkFixed = $.constraint.fixedPoint("gripperSinkFixed", { point: gripperRoute.vertices.byKey.sink, target: [156, -38] });

  const visionRoute = $.geometry.polyline("visionRoute", {
    vertices: [
      { key: "source", position: [-156, -62] },
      { key: "entry", position: [-132, -62] },
      { key: "strainReliefA", position: [-114, -74] },
      { key: "leftGuide", position: [-84, -74] },
      { key: "leftTurn", position: [-64, -58] },
      { key: "midspan", position: [-24, -58] },
      { key: "rightTurn", position: [18, -70] },
      { key: "rightGuide", position: [58, -70] },
      { key: "strainReliefB", position: [120, -62] },
      { key: "sink", position: [156, -62] },
    ],
    closed: false,
  });
  const visionSourceFixed = $.constraint.fixedPoint("visionSourceFixed", { point: visionRoute.vertices.byKey.source, target: [-156, -62] });
  const visionSinkFixed = $.constraint.fixedPoint("visionSinkFixed", { point: visionRoute.vertices.byKey.sink, target: [156, -62] });

  // Every route endpoint is anchored to its connector bank. Interior vertices
  // deliberately retain solver-instance mobility; `serviceLoop` is the focused
  // repeated drag/release target while both radii remain source-authoritative.
  const serviceRoute = $.geometry.polyline("serviceRoute", {
    vertices: [
      { key: "source", position: [-156, -86] },
      { key: "entry", position: [-136, -86] },
      { key: "strainReliefA", position: [-118, -98] },
      { key: "leftGuide", position: [-90, -98] },
      { key: "leftTurn", position: [-70, -82] },
      { key: "serviceLoop", position: [-30, -82] },
      { key: "rightTurn", position: [12, -94] },
      { key: "rightGuide", position: [54, -94] },
      { key: "strainReliefB", position: [118, -86] },
      { key: "sink", position: [156, -86] },
    ],
    closed: false,
  });
  const serviceSourceFixed = $.constraint.fixedPoint("serviceSourceFixed", { point: serviceRoute.vertices.byKey.source, target: [-156, -86] });
  const serviceSinkFixed = $.constraint.fixedPoint("serviceSinkFixed", { point: serviceRoute.vertices.byKey.sink, target: [156, -86] });

  // These two lexical literals are the only cross-route presentation controls.
  // Replacing one invocation argument with a direct literal cleanly partitions
  // its local consumers without coupling route topology or point dragging.
  const sharedClipRadius = mm(2.4);
  const sharedBendRadius = mm(5);
  const powerHarness = $.use("powerHarness", harnessRoute, { vertices: powerRoute.vertices, corners: powerRoute.filletableCorners, clipRadius: sharedClipRadius, bendRadius: sharedBendRadius });
  const servoAHarness = $.use("servoAHarness", harnessRoute, { vertices: servoARoute.vertices, corners: servoARoute.filletableCorners, clipRadius: sharedClipRadius, bendRadius: sharedBendRadius });
  const servoBHarness = $.use("servoBHarness", harnessRoute, { vertices: servoBRoute.vertices, corners: servoBRoute.filletableCorners, clipRadius: sharedClipRadius, bendRadius: sharedBendRadius });
  const sensorAHarness = $.use("sensorAHarness", harnessRoute, { vertices: sensorARoute.vertices, corners: sensorARoute.filletableCorners, clipRadius: sharedClipRadius, bendRadius: sharedBendRadius });
  const sensorBHarness = $.use("sensorBHarness", harnessRoute, { vertices: sensorBRoute.vertices, corners: sensorBRoute.filletableCorners, clipRadius: sharedClipRadius, bendRadius: sharedBendRadius });
  const gripperHarness = $.use("gripperHarness", harnessRoute, { vertices: gripperRoute.vertices, corners: gripperRoute.filletableCorners, clipRadius: sharedClipRadius, bendRadius: sharedBendRadius });
  const visionHarness = $.use("visionHarness", harnessRoute, { vertices: visionRoute.vertices, corners: visionRoute.filletableCorners, clipRadius: sharedClipRadius, bendRadius: sharedBendRadius });
  const serviceHarness = $.use("serviceHarness", harnessRoute, { vertices: serviceRoute.vertices, corners: serviceRoute.filletableCorners, clipRadius: sharedClipRadius, bendRadius: sharedBendRadius });

  $.organize("Fixture board", [board, boreNw, boreNe, boreSe, boreSw]);
  $.organize("Connector banks", [leftPower, rightPower, leftServoA, rightServoA, leftServoB, rightServoB, leftSensorA, rightSensorA, leftSensorB, rightSensorB, leftGripper, rightGripper, leftVision, rightVision, leftService, rightService]);
  $.organize("Harness routes", [powerRoute, servoARoute, servoBRoute, sensorARoute, sensorBRoute, gripperRoute, visionRoute, serviceRoute]);
  $.organize("Adaptive clips and bends", [powerHarness, servoAHarness, servoBHarness, sensorAHarness, sensorBHarness, gripperHarness, visionHarness, serviceHarness]);

  return $.outputs({
    board,
    powerRoute,
    powerClips: powerHarness.clips,
    powerFillets: powerHarness.fillets,
    servoARoute,
    servoAClips: servoAHarness.clips,
    servoAFillets: servoAHarness.fillets,
    servoBRoute,
    servoBClips: servoBHarness.clips,
    servoBFillets: servoBHarness.fillets,
    sensorARoute,
    sensorAClips: sensorAHarness.clips,
    sensorAFillets: sensorAHarness.fillets,
    sensorBRoute,
    sensorBClips: sensorBHarness.clips,
    sensorBFillets: sensorBHarness.fillets,
    gripperRoute,
    gripperClips: gripperHarness.clips,
    gripperFillets: gripperHarness.fillets,
    visionRoute,
    visionClips: visionHarness.clips,
    visionFillets: visionHarness.fillets,
    serviceRoute,
    serviceClips: serviceHarness.clips,
    serviceFillets: serviceHarness.fillets,
  });
});
