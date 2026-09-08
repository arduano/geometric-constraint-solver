"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch({
  title: "CNC vacuum fixture and spoilboard plate",
  description: "A 200 x 120 mm datum-driven plate combines a gasket groove, six-port vacuum grid and symmetric workholding pattern.",
}, ($) => {
  // A reusable 200 x 120 mm fixture plate. One corner fixes the global gauge;
  // the vacuum grid and mounting pattern are located by symmetry and datums.
  const plate = $.geometry.twoPointAlignedRectangle("plate", {
    firstCorner: [-100, -60],
    oppositeCorner: [100, 60],
    label: "Fixture plate envelope",
    role: "profile",
  });
  const plateAnchor = $.constraint.fixedPoint("plateAnchor", {
    point: plate.corners[0],
    target: [-100, -60],
    label: "Fixture origin",
  });
  const plateWidth = $.dimension.curveLength("plateWidth", {
    isKeyConstraint: true,
    curve: plate.spans[0],
    value: mm(200),
    label: "Plate width",
    mode: "driving",
  });
  const plateHeight = $.dimension.curveLength("plateHeight", {
    isKeyConstraint: true,
    curve: plate.spans[1],
    value: mm(120),
    label: "Plate height",
    mode: "driving",
  });
  const plateDiagonal = $.geometry.segment("plateDiagonal", {
    start: plate.corners[0],
    end: plate.corners[2],
    branchDirection: [0.8574929257125441, 0.5144957554275265],
    label: "Plate-centre diagonal",
    role: "construction",
  });
  const plateCenter = $.geometry.sketchPoint("plateCenter", {
    point: [0, 0],
    label: "Vacuum manifold centre",
    role: "construction",
  });
  const centerOnDiagonal = $.constraint.midpoint("centerOnDiagonal", {
    point: plateCenter.point,
    line: plateDiagonal.span,
    label: "Plate centre",
  });
  const centerPort = $.geometry.centerRadiusCircle("centerPort", {
    center: plateCenter.point,
    radius: mm(8),
    label: "Central vacuum port",
    role: "profile",
  });
  const gasket = $.geometry.twoPointAlignedRectangle("gasket", {
    firstCorner: [-90, -50],
    oppositeCorner: [90, 50],
    label: "Gasket groove centreline",
    role: "profile",
  });
  const gasketDiagonal = $.geometry.segment("gasketDiagonal", {
    start: gasket.corners[0],
    end: gasket.corners[2],
    branchDirection: [0.8741572761215377, 0.48564293117863205],
    role: "construction",
  });
  const gasketCentered = $.constraint.midpoint("gasketCentered", {
    point: plateCenter.point,
    line: gasketDiagonal.span,
    label: "Gasket centered on vacuum axis",
  });
  const gasketWidth = $.dimension.curveLength("gasketWidth", {
    isKeyConstraint: true,
    curve: gasket.spans[0],
    value: mm(180),
    label: "Gasket width",
    mode: "driving",
  });
  const gasketHeight = $.dimension.curveLength("gasketHeight", {
    isKeyConstraint: true,
    curve: gasket.spans[1],
    value: mm(100),
    label: "Gasket height",
    mode: "driving",
  });
  const vacuumUpperLeft = $.geometry.sketchPoint("vacuumUpperLeft", {
    point: [-50, 25],
    label: "Upper-left vacuum port",
    role: "construction",
  });
  const vacuumUpperCenter = $.geometry.sketchPoint("vacuumUpperCenter", {
    point: [0, 25],
    label: "Upper-centre vacuum port",
    role: "construction",
  });
  const vacuumUpperRight = $.geometry.sketchPoint("vacuumUpperRight", {
    point: [50, 25],
    label: "Upper-right vacuum port",
    role: "construction",
  });
  const vacuumLowerLeft = $.geometry.sketchPoint("vacuumLowerLeft", {
    point: [-50, -25],
    label: "Lower-left vacuum port",
    role: "construction",
  });
  const vacuumLowerCenter = $.geometry.sketchPoint("vacuumLowerCenter", {
    point: [0, -25],
    label: "Lower-centre vacuum port",
    role: "construction",
  });
  const vacuumLowerRight = $.geometry.sketchPoint("vacuumLowerRight", {
    point: [50, -25],
    label: "Lower-right vacuum port",
    role: "construction",
  });
  const upperPairSymmetry = $.constraint.symmetricAboutDatumAxis("upperPairSymmetry", {
    first: vacuumUpperLeft.point,
    second: vacuumUpperRight.point,
    axis: "y",
    label: "Upper port symmetry",
  });
  const upperRightX = $.constraint.fixedCoordinate("upperRightX", {
    point: vacuumUpperRight.point,
    axis: "x",
    target: mm(50),
    label: "Half vacuum pitch X",
  });
  const upperRightY = $.constraint.fixedCoordinate("upperRightY", {
    point: vacuumUpperRight.point,
    axis: "y",
    target: mm(25),
    label: "Half vacuum pitch Y",
  });
  const upperCenterOnY = $.constraint.pointOnDatumAxis("upperCenterOnY", {
    point: vacuumUpperCenter.point,
    axis: "y",
    label: "Upper-centre port on Y datum",
  });
  const upperRowAligned = $.constraint.horizontalPoints("upperRowAligned", {
    first: vacuumUpperLeft.point,
    second: vacuumUpperCenter.point,
    label: "Upper row alignment",
  });
  const mirrorLeftPorts = $.constraint.symmetricAboutDatumAxis("mirrorLeftPorts", {
    first: vacuumLowerLeft.point,
    second: vacuumUpperLeft.point,
    axis: "x",
    label: "Left port pair symmetry",
  });
  const mirrorCenterPorts = $.constraint.symmetricAboutDatumAxis("mirrorCenterPorts", {
    first: vacuumLowerCenter.point,
    second: vacuumUpperCenter.point,
    axis: "x",
    label: "Centre port pair symmetry",
  });
  const mirrorRightPorts = $.constraint.symmetricAboutDatumAxis("mirrorRightPorts", {
    first: vacuumLowerRight.point,
    second: vacuumUpperRight.point,
    axis: "x",
    label: "Right port pair symmetry",
  });
  const portUpperLeft = $.geometry.centerRadiusCircle("portUpperLeft", {
    center: vacuumUpperLeft.point,
    radius: mm(4),
    label: "UL vacuum port",
    role: "profile",
  });
  const portUpperCenter = $.geometry.centerRadiusCircle("portUpperCenter", {
    center: vacuumUpperCenter.point,
    radius: mm(4),
    label: "UC vacuum port",
    role: "profile",
  });
  const portUpperRight = $.geometry.centerRadiusCircle("portUpperRight", {
    center: vacuumUpperRight.point,
    radius: mm(4),
    label: "UR vacuum port",
    role: "profile",
  });
  const portLowerLeft = $.geometry.centerRadiusCircle("portLowerLeft", {
    center: vacuumLowerLeft.point,
    radius: mm(4),
    label: "LL vacuum port",
    role: "profile",
  });
  const portLowerCenter = $.geometry.centerRadiusCircle("portLowerCenter", {
    center: vacuumLowerCenter.point,
    radius: mm(4),
    label: "LC vacuum port",
    role: "profile",
  });
  const portLowerRight = $.geometry.centerRadiusCircle("portLowerRight", {
    center: vacuumLowerRight.point,
    radius: mm(4),
    label: "LR vacuum port",
    role: "profile",
  });
  const upperVacuumRail = $.geometry.segment("upperVacuumRail", {
    start: vacuumUpperLeft.point,
    end: vacuumUpperRight.point,
    branchDirection: [1, 0],
    label: "Upper vacuum rail",
    role: "construction",
  });
  const lowerVacuumRail = $.geometry.segment("lowerVacuumRail", {
    start: vacuumLowerLeft.point,
    end: vacuumLowerRight.point,
    branchDirection: [1, 0],
    label: "Lower vacuum rail",
    role: "construction",
  });
  const centerVacuumRail = $.geometry.segment("centerVacuumRail", {
    start: vacuumLowerCenter.point,
    end: vacuumUpperCenter.point,
    branchDirection: [0, 1],
    label: "Central vacuum rail",
    role: "construction",
  });
  // Through fasteners stay outside the evacuated area, with positive gasket clearance.
  const mountNe = $.geometry.sketchPoint("mountNe", {
    point: [94, 54],
    label: "NE workholding centre",
    role: "construction",
  });
  const mountNw = $.geometry.sketchPoint("mountNw", {
    point: [-94, 54],
    label: "NW workholding centre",
    role: "construction",
  });
  const mountSe = $.geometry.sketchPoint("mountSe", {
    point: [94, -54],
    label: "SE workholding centre",
    role: "construction",
  });
  const mountSw = $.geometry.sketchPoint("mountSw", {
    point: [-94, -54],
    label: "SW workholding centre",
    role: "construction",
  });
  const mountNeX = $.constraint.fixedCoordinate("mountNeX", {
    point: mountNe.point,
    axis: "x",
    target: mm(94),
    label: "Workholding X inset",
  });
  const mountNeY = $.constraint.fixedCoordinate("mountNeY", {
    point: mountNe.point,
    axis: "y",
    target: mm(54),
    label: "Workholding Y inset",
  });
  const mountNorthSymmetry = $.constraint.symmetricAboutDatumAxis("mountNorthSymmetry", {
    first: mountNw.point,
    second: mountNe.point,
    axis: "y",
    label: "North workholding symmetry",
  });
  const mountEastSymmetry = $.constraint.symmetricAboutDatumAxis("mountEastSymmetry", {
    first: mountSe.point,
    second: mountNe.point,
    axis: "x",
    label: "East workholding symmetry",
  });
  const mountWestSymmetry = $.constraint.symmetricAboutDatumAxis("mountWestSymmetry", {
    first: mountSw.point,
    second: mountNw.point,
    axis: "x",
    label: "West workholding symmetry",
  });
  const holeNe = $.geometry.centerRadiusCircle("holeNe", {
    center: mountNe.point,
    radius: mm(3.25),
    label: "NE workholding hole",
    role: "profile",
  });
  const holeNw = $.geometry.centerRadiusCircle("holeNw", {
    center: mountNw.point,
    radius: mm(3.25),
    label: "NW workholding hole",
    role: "profile",
  });
  const holeSe = $.geometry.centerRadiusCircle("holeSe", {
    center: mountSe.point,
    radius: mm(3.25),
    label: "SE workholding hole",
    role: "profile",
  });
  const holeSw = $.geometry.centerRadiusCircle("holeSw", {
    center: mountSw.point,
    radius: mm(3.25),
    label: "SW workholding hole",
    role: "profile",
  });
  const centerPortRadius = $.dimension.radius("centerPortRadius", {
    isKeyConstraint: true,
    curve: centerPort.curve,
    value: mm(8),
    label: "Central port radius",
    mode: "driving",
  });
  const upperLeftPortRadius = $.dimension.radius("upperLeftPortRadius", {
    curve: portUpperLeft.curve,
    value: mm(4),
    label: "UL port radius",
    mode: "driving",
  });
  const upperCenterPortRadius = $.dimension.radius("upperCenterPortRadius", {
    curve: portUpperCenter.curve,
    value: mm(4),
    label: "UC port radius",
    mode: "driving",
  });
  const upperRightPortRadius = $.dimension.radius("upperRightPortRadius", {
    curve: portUpperRight.curve,
    value: mm(4),
    label: "UR port radius",
    mode: "driving",
  });
  const lowerLeftPortRadius = $.dimension.radius("lowerLeftPortRadius", {
    curve: portLowerLeft.curve,
    value: mm(4),
    label: "LL port radius",
    mode: "driving",
  });
  const lowerCenterPortRadius = $.dimension.radius("lowerCenterPortRadius", {
    curve: portLowerCenter.curve,
    value: mm(4),
    label: "LC port radius",
    mode: "driving",
  });
  const lowerRightPortRadius = $.dimension.radius("lowerRightPortRadius", {
    curve: portLowerRight.curve,
    value: mm(4),
    label: "LR port radius",
    mode: "driving",
  });
  const holeNeRadius = $.dimension.radius("holeNeRadius", {
    isKeyConstraint: true,
    curve: holeNe.curve,
    value: mm(3.25),
    label: "NE workholding radius",
    mode: "driving",
  });
  const holeNwRadius = $.dimension.radius("holeNwRadius", {
    curve: holeNw.curve,
    value: mm(3.25),
    label: "NW workholding radius",
    mode: "driving",
  });
  const holeSeRadius = $.dimension.radius("holeSeRadius", {
    curve: holeSe.curve,
    value: mm(3.25),
    label: "SE workholding radius",
    mode: "driving",
  });
  const holeSwRadius = $.dimension.radius("holeSwRadius", {
    curve: holeSw.curve,
    value: mm(3.25),
    label: "SW workholding radius",
    mode: "driving",
  });
  $.group("Fixture envelope", [plate, plateAnchor, plateWidth, plateHeight, plateDiagonal, plateCenter, centerOnDiagonal, centerPort, centerPortRadius]);
  $.group("Gasket groove", [gasket, gasketDiagonal, gasketCentered, gasketWidth, gasketHeight]);
  $.group("Vacuum distribution grid", [vacuumUpperLeft, vacuumUpperCenter, vacuumUpperRight, vacuumLowerLeft, vacuumLowerCenter, vacuumLowerRight, upperPairSymmetry, upperRightX, upperRightY, upperCenterOnY, upperRowAligned, mirrorLeftPorts, mirrorCenterPorts, mirrorRightPorts, portUpperLeft, portUpperCenter, portUpperRight, portLowerLeft, portLowerCenter, portLowerRight, upperVacuumRail, lowerVacuumRail, centerVacuumRail, upperLeftPortRadius, upperCenterPortRadius, upperRightPortRadius, lowerLeftPortRadius, lowerCenterPortRadius, lowerRightPortRadius]);
  $.group("Workholding pattern", [mountNe, mountNw, mountSe, mountSw, mountNeX, mountNeY, mountNorthSymmetry, mountEastSymmetry, mountWestSymmetry, holeNe, holeNw, holeSe, holeSw, holeNeRadius, holeNwRadius, holeSeRadius, holeSwRadius]);
  return {};
});
