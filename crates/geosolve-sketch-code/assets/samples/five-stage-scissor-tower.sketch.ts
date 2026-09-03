"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1TowerLevel0Left = $.geometry.sketchPoint("point1TowerLevel0Left", {
    point: [-4, 0],
    label: "Tower level 0 left",
  });
  const point2TowerLevel0Right = $.geometry.sketchPoint("point2TowerLevel0Right", {
    point: [4, 0],
    label: "Tower level 0 right",
  });
  const point3TowerLevel1Left = $.geometry.sketchPoint("point3TowerLevel1Left", {
    point: [-4, 6],
    label: "Tower level 1 left",
  });
  const point4TowerLevel1Right = $.geometry.sketchPoint("point4TowerLevel1Right", {
    point: [4, 6],
    label: "Tower level 1 right",
  });
  const point5TowerLevel2Left = $.geometry.sketchPoint("point5TowerLevel2Left", {
    point: [-4, 12],
    label: "Tower level 2 left",
  });
  const point6TowerLevel2Right = $.geometry.sketchPoint("point6TowerLevel2Right", {
    point: [4, 12],
    label: "Tower level 2 right",
  });
  const point7TowerLevel3Left = $.geometry.sketchPoint("point7TowerLevel3Left", {
    point: [-4, 18],
    label: "Tower level 3 left",
  });
  const point8TowerLevel3Right = $.geometry.sketchPoint("point8TowerLevel3Right", {
    point: [4, 18],
    label: "Tower level 3 right",
  });
  const point9TowerLevel4Left = $.geometry.sketchPoint("point9TowerLevel4Left", {
    point: [-4, 24],
    label: "Tower level 4 left",
  });
  const point10TowerLevel4Right = $.geometry.sketchPoint("point10TowerLevel4Right", {
    point: [4, 24],
    label: "Tower level 4 right",
  });
  const point11TowerLevel5Left = $.geometry.sketchPoint("point11TowerLevel5Left", {
    point: [-4, 30],
    label: "Tower level 5 left",
  });
  const point12TowerLevel5Right = $.geometry.sketchPoint("point12TowerLevel5Right", {
    point: [4, 30],
    label: "Tower level 5 right",
  });
  const curve1TowerPlatformLevel0 = $.geometry.segment("curve1TowerPlatformLevel0", {
    start: point1TowerLevel0Left.point,
    end: point2TowerLevel0Right.point,
    branchDirection: [1, 0],
    label: "Tower platform level 0",
    role: "profile",
  });
  const curve2TowerPlatformLevel1 = $.geometry.segment("curve2TowerPlatformLevel1", {
    start: point3TowerLevel1Left.point,
    end: point4TowerLevel1Right.point,
    branchDirection: [1, 0],
    label: "Tower platform level 1",
    role: "profile",
  });
  const curve3TowerPlatformLevel2 = $.geometry.segment("curve3TowerPlatformLevel2", {
    start: point5TowerLevel2Left.point,
    end: point6TowerLevel2Right.point,
    branchDirection: [1, 0],
    label: "Tower platform level 2",
    role: "profile",
  });
  const curve4TowerPlatformLevel3 = $.geometry.segment("curve4TowerPlatformLevel3", {
    start: point7TowerLevel3Left.point,
    end: point8TowerLevel3Right.point,
    branchDirection: [1, 0],
    label: "Tower platform level 3",
    role: "profile",
  });
  const curve5TowerPlatformLevel4 = $.geometry.segment("curve5TowerPlatformLevel4", {
    start: point9TowerLevel4Left.point,
    end: point10TowerLevel4Right.point,
    branchDirection: [1, 0],
    label: "Tower platform level 4",
    role: "profile",
  });
  const curve6TowerPlatformLevel5 = $.geometry.segment("curve6TowerPlatformLevel5", {
    start: point11TowerLevel5Left.point,
    end: point12TowerLevel5Right.point,
    branchDirection: [1, 0],
    label: "Tower platform level 5",
    role: "profile",
  });
  const curve7TowerStage1RisingRightBar = $.geometry.segment("curve7TowerStage1RisingRightBar", {
    start: point1TowerLevel0Left.point,
    end: point4TowerLevel1Right.point,
    branchDirection: [0.8, 0.6],
    label: "Tower stage 1 rising-right bar",
    role: "profile",
  });
  const curve8TowerStage1RisingLeftBar = $.geometry.segment("curve8TowerStage1RisingLeftBar", {
    start: point2TowerLevel0Right.point,
    end: point3TowerLevel1Left.point,
    branchDirection: [-0.8, 0.6],
    label: "Tower stage 1 rising-left bar",
    role: "profile",
  });
  const curve9TowerStage2RisingRightBar = $.geometry.segment("curve9TowerStage2RisingRightBar", {
    start: point3TowerLevel1Left.point,
    end: point6TowerLevel2Right.point,
    branchDirection: [0.8, 0.6],
    label: "Tower stage 2 rising-right bar",
    role: "profile",
  });
  const curve10TowerStage2RisingLeftBar = $.geometry.segment("curve10TowerStage2RisingLeftBar", {
    start: point4TowerLevel1Right.point,
    end: point5TowerLevel2Left.point,
    branchDirection: [-0.8, 0.6],
    label: "Tower stage 2 rising-left bar",
    role: "profile",
  });
  const curve11TowerStage3RisingRightBar = $.geometry.segment("curve11TowerStage3RisingRightBar", {
    start: point5TowerLevel2Left.point,
    end: point8TowerLevel3Right.point,
    branchDirection: [0.8, 0.6],
    label: "Tower stage 3 rising-right bar",
    role: "profile",
  });
  const curve12TowerStage3RisingLeftBar = $.geometry.segment("curve12TowerStage3RisingLeftBar", {
    start: point6TowerLevel2Right.point,
    end: point7TowerLevel3Left.point,
    branchDirection: [-0.8, 0.6],
    label: "Tower stage 3 rising-left bar",
    role: "profile",
  });
  const curve13TowerStage4RisingRightBar = $.geometry.segment("curve13TowerStage4RisingRightBar", {
    start: point7TowerLevel3Left.point,
    end: point10TowerLevel4Right.point,
    branchDirection: [0.8, 0.6],
    label: "Tower stage 4 rising-right bar",
    role: "profile",
  });
  const curve14TowerStage4RisingLeftBar = $.geometry.segment("curve14TowerStage4RisingLeftBar", {
    start: point8TowerLevel3Right.point,
    end: point9TowerLevel4Left.point,
    branchDirection: [-0.8, 0.6],
    label: "Tower stage 4 rising-left bar",
    role: "profile",
  });
  const curve15TowerStage5RisingRightBar = $.geometry.segment("curve15TowerStage5RisingRightBar", {
    start: point9TowerLevel4Left.point,
    end: point12TowerLevel5Right.point,
    branchDirection: [0.8, 0.6],
    label: "Tower stage 5 rising-right bar",
    role: "profile",
  });
  const curve16TowerStage5RisingLeftBar = $.geometry.segment("curve16TowerStage5RisingLeftBar", {
    start: point10TowerLevel4Right.point,
    end: point11TowerLevel5Left.point,
    branchDirection: [-0.8, 0.6],
    label: "Tower stage 5 rising-left bar",
    role: "profile",
  });
  const constraint1TowerBaseLeftFixed = $.constraint.fixedPoint("constraint1TowerBaseLeftFixed", {
    point: point1TowerLevel0Left.point,
    target: [-4, 0],
    label: "Tower base left fixed",
  });
  const constraint2TowerBaseRightSlidesHorizontally = $.constraint.fixedCoordinate("constraint2TowerBaseRightSlidesHorizontally", {
    point: point2TowerLevel0Right.point,
    axis: "y",
    target: mm(0),
    label: "Tower base right slides horizontally",
  });
  const constraint3TowerPlatform1RemainsHorizontal = $.constraint.horizontal("constraint3TowerPlatform1RemainsHorizontal", {
    span: curve2TowerPlatformLevel1.span,
    label: "Tower platform 1 remains horizontal",
  });
  const constraint4TowerPlatform1MatchesBaseWidth = $.constraint.equalLength("constraint4TowerPlatform1MatchesBaseWidth", {
    first: curve1TowerPlatformLevel0.span,
    second: curve2TowerPlatformLevel1.span,
    label: "Tower platform 1 matches base width",
  });
  const constraint5TowerPlatform2RemainsHorizontal = $.constraint.horizontal("constraint5TowerPlatform2RemainsHorizontal", {
    span: curve3TowerPlatformLevel2.span,
    label: "Tower platform 2 remains horizontal",
  });
  const constraint6TowerPlatform2MatchesBaseWidth = $.constraint.equalLength("constraint6TowerPlatform2MatchesBaseWidth", {
    first: curve1TowerPlatformLevel0.span,
    second: curve3TowerPlatformLevel2.span,
    label: "Tower platform 2 matches base width",
  });
  const constraint7TowerPlatform3RemainsHorizontal = $.constraint.horizontal("constraint7TowerPlatform3RemainsHorizontal", {
    span: curve4TowerPlatformLevel3.span,
    label: "Tower platform 3 remains horizontal",
  });
  const constraint8TowerPlatform3MatchesBaseWidth = $.constraint.equalLength("constraint8TowerPlatform3MatchesBaseWidth", {
    first: curve1TowerPlatformLevel0.span,
    second: curve4TowerPlatformLevel3.span,
    label: "Tower platform 3 matches base width",
  });
  const constraint9TowerPlatform4RemainsHorizontal = $.constraint.horizontal("constraint9TowerPlatform4RemainsHorizontal", {
    span: curve5TowerPlatformLevel4.span,
    label: "Tower platform 4 remains horizontal",
  });
  const constraint10TowerPlatform4MatchesBaseWidth = $.constraint.equalLength("constraint10TowerPlatform4MatchesBaseWidth", {
    first: curve1TowerPlatformLevel0.span,
    second: curve5TowerPlatformLevel4.span,
    label: "Tower platform 4 matches base width",
  });
  const constraint11TowerPlatform5RemainsHorizontal = $.constraint.horizontal("constraint11TowerPlatform5RemainsHorizontal", {
    span: curve6TowerPlatformLevel5.span,
    label: "Tower platform 5 remains horizontal",
  });
  const constraint12TowerPlatform5MatchesBaseWidth = $.constraint.equalLength("constraint12TowerPlatform5MatchesBaseWidth", {
    first: curve1TowerPlatformLevel0.span,
    second: curve6TowerPlatformLevel5.span,
    label: "Tower platform 5 matches base width",
  });
  const dimension1TowerMasterDiagonalLength10 = $.dimension.curveLength("dimension1TowerMasterDiagonalLength10", {
    curve: curve7TowerStage1RisingRightBar.span,
    value: mm(10),
    label: "Tower master diagonal length 10",
    mode: "driving",
  });
  const constraint13TowerDiagonal2MatchesMaster = $.constraint.equalLength("constraint13TowerDiagonal2MatchesMaster", {
    first: curve7TowerStage1RisingRightBar.span,
    second: curve8TowerStage1RisingLeftBar.span,
    label: "Tower diagonal 2 matches master",
  });
  const constraint14TowerDiagonal3MatchesMaster = $.constraint.equalLength("constraint14TowerDiagonal3MatchesMaster", {
    first: curve7TowerStage1RisingRightBar.span,
    second: curve9TowerStage2RisingRightBar.span,
    label: "Tower diagonal 3 matches master",
  });
  const constraint15TowerDiagonal4MatchesMaster = $.constraint.equalLength("constraint15TowerDiagonal4MatchesMaster", {
    first: curve7TowerStage1RisingRightBar.span,
    second: curve10TowerStage2RisingLeftBar.span,
    label: "Tower diagonal 4 matches master",
  });
  const constraint16TowerDiagonal5MatchesMaster = $.constraint.equalLength("constraint16TowerDiagonal5MatchesMaster", {
    first: curve7TowerStage1RisingRightBar.span,
    second: curve11TowerStage3RisingRightBar.span,
    label: "Tower diagonal 5 matches master",
  });
  const constraint17TowerDiagonal6MatchesMaster = $.constraint.equalLength("constraint17TowerDiagonal6MatchesMaster", {
    first: curve7TowerStage1RisingRightBar.span,
    second: curve12TowerStage3RisingLeftBar.span,
    label: "Tower diagonal 6 matches master",
  });
  const constraint18TowerDiagonal7MatchesMaster = $.constraint.equalLength("constraint18TowerDiagonal7MatchesMaster", {
    first: curve7TowerStage1RisingRightBar.span,
    second: curve13TowerStage4RisingRightBar.span,
    label: "Tower diagonal 7 matches master",
  });
  const constraint19TowerDiagonal8MatchesMaster = $.constraint.equalLength("constraint19TowerDiagonal8MatchesMaster", {
    first: curve7TowerStage1RisingRightBar.span,
    second: curve14TowerStage4RisingLeftBar.span,
    label: "Tower diagonal 8 matches master",
  });
  const constraint20TowerDiagonal9MatchesMaster = $.constraint.equalLength("constraint20TowerDiagonal9MatchesMaster", {
    first: curve7TowerStage1RisingRightBar.span,
    second: curve15TowerStage5RisingRightBar.span,
    label: "Tower diagonal 9 matches master",
  });
  const constraint21TowerDiagonal10MatchesMaster = $.constraint.equalLength("constraint21TowerDiagonal10MatchesMaster", {
    first: curve7TowerStage1RisingRightBar.span,
    second: curve16TowerStage5RisingLeftBar.span,
    label: "Tower diagonal 10 matches master",
  });
  $.group("Points", [point1TowerLevel0Left, point2TowerLevel0Right, point3TowerLevel1Left, point4TowerLevel1Right, point5TowerLevel2Left, point6TowerLevel2Right, point7TowerLevel3Left, point8TowerLevel3Right, point9TowerLevel4Left, point10TowerLevel4Right, point11TowerLevel5Left, point12TowerLevel5Right]);
  $.group("Geometry", [curve1TowerPlatformLevel0, curve2TowerPlatformLevel1, curve3TowerPlatformLevel2, curve4TowerPlatformLevel3, curve5TowerPlatformLevel4, curve6TowerPlatformLevel5, curve7TowerStage1RisingRightBar, curve8TowerStage1RisingLeftBar, curve9TowerStage2RisingRightBar, curve10TowerStage2RisingLeftBar, curve11TowerStage3RisingRightBar, curve12TowerStage3RisingLeftBar, curve13TowerStage4RisingRightBar, curve14TowerStage4RisingLeftBar, curve15TowerStage5RisingRightBar, curve16TowerStage5RisingLeftBar]);
  $.group("Constraints", [constraint1TowerBaseLeftFixed, constraint2TowerBaseRightSlidesHorizontally, constraint3TowerPlatform1RemainsHorizontal, constraint4TowerPlatform1MatchesBaseWidth, constraint5TowerPlatform2RemainsHorizontal, constraint6TowerPlatform2MatchesBaseWidth, constraint7TowerPlatform3RemainsHorizontal, constraint8TowerPlatform3MatchesBaseWidth, constraint9TowerPlatform4RemainsHorizontal, constraint10TowerPlatform4MatchesBaseWidth, constraint11TowerPlatform5RemainsHorizontal, constraint12TowerPlatform5MatchesBaseWidth, constraint13TowerDiagonal2MatchesMaster, constraint14TowerDiagonal3MatchesMaster, constraint15TowerDiagonal4MatchesMaster, constraint16TowerDiagonal5MatchesMaster, constraint17TowerDiagonal6MatchesMaster, constraint18TowerDiagonal7MatchesMaster, constraint19TowerDiagonal8MatchesMaster, constraint20TowerDiagonal9MatchesMaster, constraint21TowerDiagonal10MatchesMaster]);
  $.group("Dimensions", [dimension1TowerMasterDiagonalLength10]);
  return {};
});
