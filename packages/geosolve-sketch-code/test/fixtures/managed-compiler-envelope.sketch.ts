// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import {sketch,mm} from "@geosolve/sketch-code";

export default sketch(($)=>{
  // 多字节 source-site and normalization parity.
  const sharedRadius=mm(4);
  const hole=$.geometry.centerRadiusCircle("hole",{center:[0,2],radius:sharedRadius});
  const radius=$.dimension.radius("radius",{curve:hole.curve,value:sharedRadius});
  const point=$.geometry.sketchPoint("point",{point:[1.25,-6.5],label:"Point"});
  const segment=$.geometry.segment("segment",{start:point.point,end:[9,2]});
  $.group("Canvas additions",[hole,radius,point,segment]);
  $.suppress(radius);
  return { hole, radius, point, segment };
});
