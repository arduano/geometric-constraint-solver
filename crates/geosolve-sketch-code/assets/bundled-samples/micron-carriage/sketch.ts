"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // Selected YZ interface from CNC_Carriage.step, with Z=18.89933933439
  // as the rail-pattern mid-height. The source has M3 axes at Y=+/-8
  // and Z=11.39933933439/26.39933933439 (16 x 15 pitch); the alternate
  // +/-5 axes are shown as construction. Belt fastener axes are 25 mm apart
  // at Z=33.2497800904261. The continuous stepped body is schematic.
  const carriageBody = $.geometry.polyline("carriageBody", {
    vertices: [{
      key: "p0",
      position: [-11, -15],
    }, {
      key: "p1",
      position: [11, -15],
    }, {
      key: "p2",
      position: [11, 10],
    }, {
      key: "p3",
      position: [21, 10],
    }, {
      key: "p4",
      position: [21, 25],
    }, {
      key: "p5",
      position: [-21, 25],
    }, {
      key: "p6",
      position: [-21, 10],
    }, {
      key: "p7",
      position: [-11, 10],
    }],
    closed: true,
    label: "Schematic continuous CNC body and belt wing",
    role: "profile",
  });
  const bodyEdge0 = $.constraint.horizontalPoints("bodyEdge0", {
    first: carriageBody.vertices.byKey.p0,
    second: carriageBody.vertices.byKey.p1,
    label: "Schematic body orthogonal boundary",
  });
  const bodySymmetry0 = $.constraint.symmetricAboutDatumAxis("bodySymmetry0", {
    first: carriageBody.vertices.byKey.p0,
    second: carriageBody.vertices.byKey.p1,
    axis: "y",
    label: "Body bilateral symmetry",
  });
  const bodyEdge1 = $.constraint.verticalPoints("bodyEdge1", {
    first: carriageBody.vertices.byKey.p1,
    second: carriageBody.vertices.byKey.p2,
    label: "Schematic body orthogonal boundary",
  });
  const bodyEdge2 = $.constraint.horizontalPoints("bodyEdge2", {
    first: carriageBody.vertices.byKey.p2,
    second: carriageBody.vertices.byKey.p3,
    label: "Schematic body orthogonal boundary",
  });
  const bodyEdge3 = $.constraint.verticalPoints("bodyEdge3", {
    first: carriageBody.vertices.byKey.p3,
    second: carriageBody.vertices.byKey.p4,
    label: "Schematic body orthogonal boundary",
  });
  const bodyEdge4 = $.constraint.horizontalPoints("bodyEdge4", {
    first: carriageBody.vertices.byKey.p4,
    second: carriageBody.vertices.byKey.p5,
    label: "Schematic body orthogonal boundary",
  });
  const bodyEdge5 = $.constraint.verticalPoints("bodyEdge5", {
    first: carriageBody.vertices.byKey.p5,
    second: carriageBody.vertices.byKey.p6,
    label: "Schematic body orthogonal boundary",
  });
  const bodySymmetry5 = $.constraint.symmetricAboutDatumAxis("bodySymmetry5", {
    first: carriageBody.vertices.byKey.p5,
    second: carriageBody.vertices.byKey.p4,
    axis: "y",
    label: "Body bilateral symmetry",
  });
  const bodyEdge6 = $.constraint.horizontalPoints("bodyEdge6", {
    first: carriageBody.vertices.byKey.p6,
    second: carriageBody.vertices.byKey.p7,
    label: "Schematic body orthogonal boundary",
  });
  const bodySymmetry6 = $.constraint.symmetricAboutDatumAxis("bodySymmetry6", {
    first: carriageBody.vertices.byKey.p6,
    second: carriageBody.vertices.byKey.p3,
    axis: "y",
    label: "Body bilateral symmetry",
  });
  const bodyEdge7 = $.constraint.verticalPoints("bodyEdge7", {
    first: carriageBody.vertices.byKey.p7,
    second: carriageBody.vertices.byKey.p0,
    label: "Schematic body orthogonal boundary",
  });
  const bodySymmetry7 = $.constraint.symmetricAboutDatumAxis("bodySymmetry7", {
    first: carriageBody.vertices.byKey.p7,
    second: carriageBody.vertices.byKey.p2,
    axis: "y",
    label: "Body bilateral symmetry",
  });
  const bodyHalfWidth = $.constraint.fixedCoordinate("bodyHalfWidth", {
    point: carriageBody.vertices.byKey.p1,
    axis: "x",
    target: mm(11),
    label: "Schematic bodyHalfWidth design datum",
  });
  const wingHalfWidth = $.constraint.fixedCoordinate("wingHalfWidth", {
    point: carriageBody.vertices.byKey.p3,
    axis: "x",
    target: mm(21),
    label: "Schematic wingHalfWidth design datum",
  });
  const bodyBottom = $.constraint.fixedCoordinate("bodyBottom", {
    point: carriageBody.vertices.byKey.p1,
    axis: "y",
    target: mm(-15),
    label: "Schematic bodyBottom design datum",
  });
  const wingShoulder = $.constraint.fixedCoordinate("wingShoulder", {
    point: carriageBody.vertices.byKey.p3,
    axis: "y",
    target: mm(10),
    label: "Schematic wingShoulder design datum",
  });
  const wingTop = $.constraint.fixedCoordinate("wingTop", {
    point: carriageBody.vertices.byKey.p4,
    axis: "y",
    target: mm(25),
    label: "Schematic wingTop design datum",
  });
  const railDatum = $.geometry.centerRectangle("railDatum", {
    center: [0, 0],
    corner: [8, 7.5],
    label: "Extracted M3 16 x 15 rail mounting",
    role: "construction",
  });
  const railDatumOrigin = $.constraint.fixedPoint("railDatumOrigin", {
    point: railDatum.center,
    target: [0, 0],
    label: "Extracted M3 16 x 15 rail mounting centre datum",
  });
  const railDatumWidth = $.dimension.curveLength("railDatumWidth", {
    curve: railDatum.spans[0],
    value: mm(16),
    mode: "driving",
    label: "Extracted M3 16 x 15 rail mounting width",
  });
  const railDatumHeight = $.dimension.curveLength("railDatumHeight", {
    curve: railDatum.spans[1],
    value: mm(15),
    mode: "driving",
    label: "Extracted M3 16 x 15 rail mounting height",
  });
  const railDatumHole1 = $.geometry.centerRadiusCircle("railDatumHole1", {
    center: railDatum.corners[0],
    radius: mm(1.5),
    label: "Extracted M3 16 x 15 rail mounting hole 1",
    role: "profile",
  });
  const railDatumRadius = $.dimension.radius("railDatumRadius", {
    curve: railDatumHole1.curve,
    value: mm(1.5),
    mode: "driving",
    label: "Extracted M3 16 x 15 rail mounting clearance radius",
  });
  const railDatumHole2 = $.geometry.centerRadiusCircle("railDatumHole2", {
    center: railDatum.corners[1],
    radius: mm(1.5),
    label: "Extracted M3 16 x 15 rail mounting hole 2",
    role: "profile",
  });
  const railDatumHole2Match = $.constraint.equalRadius("railDatumHole2Match", {
    first: railDatumHole1.curve,
    second: railDatumHole2.curve,
    label: "Extracted M3 16 x 15 rail mounting equal clearances",
  });
  const railDatumHole3 = $.geometry.centerRadiusCircle("railDatumHole3", {
    center: railDatum.corners[2],
    radius: mm(1.5),
    label: "Extracted M3 16 x 15 rail mounting hole 3",
    role: "profile",
  });
  const railDatumHole3Match = $.constraint.equalRadius("railDatumHole3Match", {
    first: railDatumHole1.curve,
    second: railDatumHole3.curve,
    label: "Extracted M3 16 x 15 rail mounting equal clearances",
  });
  const railDatumHole4 = $.geometry.centerRadiusCircle("railDatumHole4", {
    center: railDatum.corners[3],
    radius: mm(1.5),
    label: "Extracted M3 16 x 15 rail mounting hole 4",
    role: "profile",
  });
  const railDatumHole4Match = $.constraint.equalRadius("railDatumHole4Match", {
    first: railDatumHole1.curve,
    second: railDatumHole4.curve,
    label: "Extracted M3 16 x 15 rail mounting equal clearances",
  });
  const alternatePitch = $.operation.rectangle("alternatePitch", {
    origin: [-5, -7.5],
    width: mm(10),
    height: mm(15),
    label: "Extracted alternate 10 x 15 mounting datum",
    role: "construction",
  });
  const alternateHole0 = $.geometry.centerRadiusCircle("alternateHole0", {
    center: [-5, -7.5],
    radius: mm(1.5),
    label: "Alternate extracted M3 mounting position",
    role: "construction",
  });
  const alternateHole0Match = $.constraint.equalRadius("alternateHole0Match", {
    first: alternateHole0.curve,
    second: railDatumHole1.curve,
    label: "Same M3 mounting diameter",
  });
  const alternateHole1 = $.geometry.centerRadiusCircle("alternateHole1", {
    center: [5, -7.5],
    radius: mm(1.5),
    label: "Alternate extracted M3 mounting position",
    role: "construction",
  });
  const alternateHole1Match = $.constraint.equalRadius("alternateHole1Match", {
    first: alternateHole1.curve,
    second: railDatumHole1.curve,
    label: "Same M3 mounting diameter",
  });
  const alternateHole2 = $.geometry.centerRadiusCircle("alternateHole2", {
    center: [5, 7.5],
    radius: mm(1.5),
    label: "Alternate extracted M3 mounting position",
    role: "construction",
  });
  const alternateHole2Match = $.constraint.equalRadius("alternateHole2Match", {
    first: alternateHole2.curve,
    second: railDatumHole1.curve,
    label: "Same M3 mounting diameter",
  });
  const alternateHole3 = $.geometry.centerRadiusCircle("alternateHole3", {
    center: [-5, 7.5],
    radius: mm(1.5),
    label: "Alternate extracted M3 mounting position",
    role: "construction",
  });
  const alternateHole3Match = $.constraint.equalRadius("alternateHole3Match", {
    first: alternateHole3.curve,
    second: railDatumHole1.curve,
    label: "Same M3 mounting diameter",
  });
  const beltMountAxis = $.geometry.segment("beltMountAxis", {
    start: [-12.5, 14.3504407560362],
    end: [12.5, 14.3504407560362],
    branchDirection: [1, 0],
    role: "construction",
    label: "Extracted belt screw counterbore pitch axis",
  });
  const beltMountAligned = $.constraint.horizontal("beltMountAligned", {
    span: beltMountAxis.span,
    label: "Extracted belt screw counterbore alignment",
  });
  const beltMountCenter = $.geometry.sketchPoint("beltMountCenter", {
    point: [0, 14.3504407560362],
    role: "construction",
    label: "Extracted belt screw counterbore midpoint",
  });
  const beltMountOrigin = $.constraint.fixedPoint("beltMountOrigin", {
    point: beltMountCenter.point,
    target: [0, 14.3504407560362],
    label: "Extracted belt screw counterbore reference midpoint",
  });
  const beltMountCentered = $.constraint.midpoint("beltMountCentered", {
    point: beltMountCenter.point,
    line: beltMountAxis.span,
    label: "Extracted belt screw counterbore symmetric pitch",
  });
  const beltMountPitch = $.dimension.curveLength("beltMountPitch", {
    curve: beltMountAxis.span,
    value: mm(25),
    mode: "driving",
    label: "Extracted belt screw counterbore centre distance",
  });
  const beltMountFirst = $.geometry.centerRadiusCircle("beltMountFirst", {
    center: beltMountAxis.start,
    radius: mm(3.5),
    role: "profile",
    label: "Extracted belt screw counterbore first",
  });
  const beltMountSecond = $.geometry.centerRadiusCircle("beltMountSecond", {
    center: beltMountAxis.end,
    radius: mm(3.5),
    role: "profile",
    label: "Extracted belt screw counterbore second",
  });
  const beltMountRadius = $.dimension.radius("beltMountRadius", {
    curve: beltMountFirst.curve,
    value: mm(3.5),
    mode: "driving",
    label: "Extracted belt screw counterbore radius",
  });
  const beltMountMatched = $.constraint.equalRadius("beltMountMatched", {
    first: beltMountFirst.curve,
    second: beltMountSecond.curve,
    label: "Extracted belt screw counterbore matching radii",
  });
  const alternateHole0Located = $.constraint.coincident("alternateHole0Located", {
    first: alternateHole0.center,
    second: alternatePitch.corners.bottomLeft,
    label: "Alternate mounting datum",
  });
  const alternateHole1Located = $.constraint.coincident("alternateHole1Located", {
    first: alternateHole1.center,
    second: alternatePitch.corners.bottomRight,
    label: "Alternate mounting datum",
  });
  const alternateHole2Located = $.constraint.coincident("alternateHole2Located", {
    first: alternateHole2.center,
    second: alternatePitch.corners.topRight,
    label: "Alternate mounting datum",
  });
  const alternateHole3Located = $.constraint.coincident("alternateHole3Located", {
    first: alternateHole3.center,
    second: alternatePitch.corners.topLeft,
    label: "Alternate mounting datum",
  });
  $.group("CNC carriage body", [carriageBody, bodyEdge0, bodySymmetry0, bodyEdge1, bodyEdge2, bodyEdge3, bodyEdge4, bodyEdge5, bodySymmetry5, bodyEdge6, bodySymmetry6, bodyEdge7, bodySymmetry7, bodyHalfWidth, wingHalfWidth, bodyBottom, wingShoulder, wingTop]);
  $.group("Rail-block datum", [railDatum, railDatumOrigin, railDatumWidth, railDatumHeight, railDatumHole1, railDatumRadius, railDatumHole2, railDatumHole2Match, railDatumHole3, railDatumHole3Match, railDatumHole4, railDatumHole4Match, alternatePitch, alternateHole0, alternateHole0Match, alternateHole0Located, alternateHole1, alternateHole1Match, alternateHole1Located, alternateHole2, alternateHole2Match, alternateHole2Located, alternateHole3, alternateHole3Match, alternateHole3Located]);
  $.group("Toolhead interface", [beltMountAxis, beltMountAligned, beltMountCenter, beltMountOrigin, beltMountCentered, beltMountPitch, beltMountFirst, beltMountSecond, beltMountRadius, beltMountMatched]);
  return {};
});
